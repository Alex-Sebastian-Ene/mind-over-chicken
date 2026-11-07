use wasm_bindgen::prelude::*;

fn clamp(v: f32, lo: f32, hi: f32) -> f32 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

fn wrap_deg(deg: f32) -> f32 {
    ((deg + 540.0) % 360.0) - 180.0
}

fn norm3(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

fn normalize3(v: [f32; 3]) -> Option<[f32; 3]> {
    let n = norm3(v);
    if n > 1e-6 && n.is_finite() {
        Some([v[0] / n, v[1] / n, v[2] / n])
    } else {
        None
    }
}

fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn build_head_basis(u0: [f32; 3]) -> ([f32; 3], [f32; 3]) {
    let ref_fwd = if u0[0].abs() < 0.9 {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 0.0, -1.0]
    };
    let proj = dot3(ref_fwd, u0);
    let f_raw = [
        ref_fwd[0] - proj * u0[0],
        ref_fwd[1] - proj * u0[1],
        ref_fwd[2] - proj * u0[2],
    ];
    let f0 = normalize3(f_raw).unwrap_or([1.0, 0.0, 0.0]);
    let r0 = normalize3(cross3(f0, u0)).unwrap_or([0.0, -1.0, 0.0]);
    (f0, r0)
}

#[derive(Clone, Copy)]
struct MotionCalibSample {
    yaw: f32,
    pitch: f32,
    roll: f32,
    tx: f32,
    ty: f32,
}

fn solve_4x4(mut a: [[f32; 4]; 4], mut b: [f32; 4]) -> Option<[f32; 4]> {
    for i in 0..4 {
        let mut max_row = i;
        let mut max_val = a[i][i].abs();
        for k in (i + 1)..4 {
            if a[k][i].abs() > max_val {
                max_val = a[k][i].abs();
                max_row = k;
            }
        }
        if max_val < 1e-6 {
            return None;
        }
        if max_row != i {
            a.swap(i, max_row);
            b.swap(i, max_row);
        }
        let piv = a[i][i];
        for j in i..4 {
            a[i][j] /= piv;
        }
        b[i] /= piv;
        for k in 0..4 {
            if k != i {
                let factor = a[k][i];
                for j in i..4 {
                    a[k][j] -= factor * a[i][j];
                }
                b[k] -= factor * b[i];
            }
        }
    }
    Some(b)
}

#[wasm_bindgen]
pub struct RustImuTracker {
    up: Option<[f32; 3]>,
    u0: Option<[f32; 3]>,
    f0: [f32; 3],
    r0: [f32; 3],
    gyro_bias: [f32; 3],
    bias_calibrated: bool,
    still_sum: [f32; 3],
    still_count: u32,
    still_time: f32,
    edge_still_time: f32,
    prev_bridge_yaw: Option<f32>,
    yaw_deg: f32,
    yaw_zero_deg: f32,
    rate_deg_s: f32,
    rel_pitch: f32,
    rel_roll: f32,
    rel_yaw: f32,
    raw_pitch: f32,
    raw_roll: f32,
    raw_yaw: f32,
    tilt_x: f32,
    tilt_y: f32,
    aim_x: f32,
    aim_y: f32,
    sensitivity: f32,
    invert_yaw: bool,
    invert_pitch: bool,
    deadzone_deg: f32,
    yaw_range_deg: f32,
    pitch_range_deg: f32,
    calibrating_motion: bool,
    calib_samples: Vec<MotionCalibSample>,
    learned_wx: [f32; 4],
    learned_wy: [f32; 4],
    scale_left: f32,
    scale_right: f32,
    scale_up: f32,
    scale_down: f32,
    has_learned_map: bool,
    calib_r2: f32,
}

#[wasm_bindgen]
impl RustImuTracker {
    #[wasm_bindgen(constructor)]
    pub fn new() -> RustImuTracker {
        RustImuTracker {
            up: None,
            u0: None,
            f0: [1.0, 0.0, 0.0],
            r0: [0.0, -1.0, 0.0],
            gyro_bias: [0.0, 0.0, 0.0],
            bias_calibrated: false,
            still_sum: [0.0, 0.0, 0.0],
            still_count: 0,
            still_time: 0.0,
            edge_still_time: 0.0,
            prev_bridge_yaw: None,
            yaw_deg: 0.0,
            yaw_zero_deg: 0.0,
            rate_deg_s: 0.0,
            rel_pitch: 0.0,
            rel_roll: 0.0,
            rel_yaw: 0.0,
            raw_pitch: 0.0,
            raw_roll: 0.0,
            raw_yaw: 0.0,
            tilt_x: 0.0,
            tilt_y: 0.0,
            aim_x: 0.5,
            aim_y: 0.5,
            sensitivity: 1.0,
            invert_yaw: false,
            invert_pitch: false,
            deadzone_deg: 0.45,
            yaw_range_deg: 14.0,
            pitch_range_deg: 7.5,
            calibrating_motion: false,
            calib_samples: Vec::new(),
            learned_wx: [0.0, 1.0 / 14.0, 0.0, 0.0],
            learned_wy: [0.0, 0.0, -1.0 / 7.5, 0.0],
            scale_left: 1.0,
            scale_right: 1.0,
            scale_up: 1.0,
            scale_down: 1.0,
            has_learned_map: false,
            calib_r2: 0.0,
        }
    }

    pub fn set_config(
        &mut self,
        sensitivity: f32,
        invert_yaw: bool,
        invert_pitch: bool,
        deadzone_deg: f32,
    ) {
        self.sensitivity = clamp(sensitivity, 0.25, 4.0);
        self.invert_yaw = invert_yaw;
        self.invert_pitch = invert_pitch;
        self.deadzone_deg = clamp(deadzone_deg, 0.0, 10.0);
    }

    pub fn tare(&mut self) {
        if let Some(u) = self.up {
            let (f0, r0) = build_head_basis(u);
            self.u0 = Some(u);
            self.f0 = f0;
            self.r0 = r0;
        } else {
            self.u0 = None;
        }
        self.yaw_zero_deg = self.yaw_deg;
        self.rel_pitch = 0.0;
        self.rel_roll = 0.0;
        self.rel_yaw = 0.0;
        self.tilt_x = 0.0;
        self.tilt_y = 0.0;
        self.aim_x = 0.5;
        self.aim_y = 0.5;
        self.edge_still_time = 0.0;
        self.learned_wx[0] = 0.0;
        self.learned_wy[0] = 0.0;
    }

    pub fn begin_motion_calibration(&mut self) {
        self.tare();
        self.calibrating_motion = true;
        self.calib_samples.clear();
    }

    pub fn cancel_motion_calibration(&mut self) {
        self.calibrating_motion = false;
        self.calib_samples.clear();
    }

    pub fn reset_motion_model(&mut self) {
        self.calibrating_motion = false;
        self.calib_samples.clear();
        self.has_learned_map = false;
        self.calib_r2 = 0.0;
        self.learned_wx = [0.0, 1.0 / 14.0, 0.0, 0.0];
        self.learned_wy = [0.0, 0.0, -1.0 / 7.5, 0.0];
        self.scale_left = 1.0;
        self.scale_right = 1.0;
        self.scale_up = 1.0;
        self.scale_down = 1.0;
    }

    /// Records a supervised sample pairing the user's current 3D relative orientation
    /// `(rel_yaw, rel_pitch, rel_roll)` with the target white dot screen position `(dot_x, dot_y)` in `0..1`.
    pub fn add_calib_point(&mut self, dot_x: f32, dot_y: f32) {
        if !dot_x.is_finite() || !dot_y.is_finite() {
            return;
        }
        let tx = clamp((dot_x - 0.5) * 2.0, -1.0, 1.0);
        let ty = clamp((dot_y - 0.5) * 2.0, -1.0, 1.0);
        self.calib_samples.push(MotionCalibSample {
            yaw: self.rel_yaw,
            pitch: self.rel_pitch,
            roll: self.rel_roll,
            tx,
            ty,
        });
    }

    /// Hybrid Smooth-Pursuit + Boundary-Fixation + Center-Checkpoint Solver:
    /// 1. Extracts Center checkpoints (`|tx| < 0.08 && |ty| < 0.08`) to remove any gyro yaw drift
    ///    accumulated across calibration and guarantee zero-intercept symmetry on reverse.
    /// 2. Searches optimal human smooth-pursuit reaction lag `L in 0..=2` on moving segments while
    ///    weighting stationary boundary-hold anchors (`3.0x`) for exact screen edge locking.
    /// 3. Solves zero-prior 3D sensor-to-head Ridge Regression (`[yaw, pitch, roll] -> (u_x, u_y)`)
    ///    with cross-axis Gram-Schmidt decoupling so vertical nods never leak horizontally.
    /// 4. Computes independent 4-boundary reach envelopes (`scale_left, scale_right, scale_up, scale_down`)
    ///    so 89% of the user's calibrated edge posture reaches 100% of all 4 screen boundaries & corners.
    pub fn finish_motion_calibration(&mut self) -> bool {
        self.calibrating_motion = false;
        let n = self.calib_samples.len();
        if n < 10 {
            return false;
        }

        // Step 1: Identify Center checkpoints (`|tx| < 0.08 && |ty| < 0.08`) to detrend gyro drift
        // and guarantee that neutral Center head posture maps strictly to `(0, 0)`.
        let mut checkpoints: Vec<(f32, f32, f32, f32)> = Vec::new();
        let mut i = 0;
        while i < n {
            let s = self.calib_samples[i];
            if s.tx.abs() < 0.08 && s.ty.abs() < 0.08 {
                let start = i;
                let mut sum_y = 0.0f32;
                let mut sum_p = 0.0f32;
                let mut sum_r = 0.0f32;
                let mut cnt = 0u32;
                while i < n && self.calib_samples[i].tx.abs() < 0.08 && self.calib_samples[i].ty.abs() < 0.08 {
                    // Skip the first 2 settling samples if a center segment is long enough
                    if i >= start + 2 || start == 0 {
                        sum_y += self.calib_samples[i].yaw;
                        sum_p += self.calib_samples[i].pitch;
                        sum_r += self.calib_samples[i].roll;
                        cnt += 1;
                    }
                    i += 1;
                }
                if cnt == 0 {
                    for k in start..i {
                        sum_y += self.calib_samples[k].yaw;
                        sum_p += self.calib_samples[k].pitch;
                        sum_r += self.calib_samples[k].roll;
                        cnt += 1;
                    }
                }
                if cnt > 0 {
                    let mid = (start + i - 1) as f32 * 0.5;
                    let inv = 1.0 / (cnt as f32);
                    checkpoints.push((mid, sum_y * inv, sum_p * inv, sum_r * inv));
                }
            } else {
                i += 1;
            }
        }

        let mut detrended: Vec<MotionCalibSample> = Vec::with_capacity(n);
        for idx in 0..n {
            let s = self.calib_samples[idx];
            let (cy, cp, cr) = if checkpoints.is_empty() {
                (0.0, 0.0, 0.0)
            } else if checkpoints.len() == 1 {
                (checkpoints[0].1, checkpoints[0].2, checkpoints[0].3)
            } else {
                let x = idx as f32;
                if x <= checkpoints[0].0 {
                    (checkpoints[0].1, checkpoints[0].2, checkpoints[0].3)
                } else if x >= checkpoints[checkpoints.len() - 1].0 {
                    let last = checkpoints[checkpoints.len() - 1];
                    (last.1, last.2, last.3)
                } else {
                    let mut seg = 0;
                    for m in 0..(checkpoints.len() - 1) {
                        if x >= checkpoints[m].0 && x <= checkpoints[m + 1].0 {
                            seg = m;
                            break;
                        }
                    }
                    let c0 = checkpoints[seg];
                    let c1 = checkpoints[seg + 1];
                    let span = (c1.0 - c0.0).max(1.0);
                    let t = clamp((x - c0.0) / span, 0.0, 1.0);
                    (
                        c0.1 + t * (c1.1 - c0.1),
                        c0.2 + t * (c1.2 - c0.2),
                        c0.3 + t * (c1.3 - c0.3),
                    )
                }
            };
            detrended.push(MotionCalibSample {
                yaw: s.yaw - cy,
                pitch: s.pitch - cp,
                roll: s.roll - cr,
                tx: s.tx,
                ty: s.ty,
            });
        }

        // Update yaw_zero_deg by the final Center checkpoint drift so post-calibration starts centered
        if let Some(&(_, last_cy, _, _)) = checkpoints.last() {
            self.yaw_zero_deg = wrap_deg(self.yaw_zero_deg + last_cy);
        }

        // Step 2: Optimal pursuit phase-lag search `best_lag in 0..=2` on moving pursuit samples
        let max_lag = if n >= 30 { 2usize } else { 0usize };
        let mut best_lag = 0usize;
        let mut best_corr = -1.0f32;
        for lag in 0..=max_lag {
            let mut corr_sum = 0.0f32;
            for idx in lag..n {
                let s = detrended[idx];
                let tgt = detrended[idx - lag];
                corr_sum += (s.yaw * tgt.tx).abs() + (s.pitch * tgt.ty).abs() * 1.5;
            }
            if corr_sum > best_corr {
                best_corr = corr_sum;
                best_lag = lag;
            }
        }

        // Step 3: Weighted Zero-Intercept 3D Sensor-to-Head Regression (`[0, wyaw, wpitch, wroll]`)
        // Zero biased prior in xty so user's physical head movement 100% determines axis signs!
        let mut xtx = [[0.0f32; 4]; 4];
        let mut xty_x = [0.0f32; 4];
        let mut xty_y = [0.0f32; 4];
        xtx[0][0] = 1000.0; // Lock intercept w[0] = 0.0 so (0,0,0) is always exact screen Center
        xtx[1][1] = 0.35;
        xtx[2][2] = 0.25;
        xtx[3][3] = 6.0;

        for idx in best_lag..n {
            let s = detrended[idx];
            let prev_tgt = if idx > 0 { detrended[idx - 1] } else { s };
            let tgt_speed = (s.tx - prev_tgt.tx).hypot(s.ty - prev_tgt.ty);
            let is_boundary_hold = (s.tx.abs() > 0.72 || s.ty.abs() > 0.72) && tgt_speed < 0.02;
            let tgt = if is_boundary_hold { s } else { detrended[idx - best_lag] };

            // Up-weight stationary boundary holds so edges and corners have strong anchoring
            let weight = if is_boundary_hold {
                3.0
            } else if tgt.tx.abs() > 0.70 || tgt.ty.abs() > 0.70 {
                1.8
            } else {
                1.0
            };

            let f = [0.0, s.yaw, s.pitch, s.roll];
            for r in 1..4 {
                for c in 1..4 {
                    xtx[r][c] += weight * f[r] * f[c];
                }
                xty_x[r] += weight * f[r] * tgt.tx;
                xty_y[r] += weight * f[r] * tgt.ty;
            }
        }

        let wx = match solve_4x4(xtx, xty_x) {
            Some(w) => w,
            None => return false,
        };
        let wy = match solve_4x4(xtx, xty_y) {
            Some(w) => w,
            None => return false,
        };

        let mut clean_wx = [0.0, wx[1], wx[2], wx[3]];
        let mut clean_wy = [0.0, wy[1], wy[2], wy[3]];

        // Suppress cross-axis leakage if pure horizontal turns and pure vertical nods are present
        let wx_dom_yaw = clean_wx[1].abs() >= clean_wx[2].abs();
        let wy_dom_pitch = clean_wy[2].abs() >= clean_wy[1].abs();
        if wx_dom_yaw && wy_dom_pitch {
            // Limit cross-talk between yaw and pitch to <= 25% of primary axis weight
            let max_cross_x = clean_wx[1].abs() * 0.25;
            let max_cross_y = clean_wy[2].abs() * 0.25;
            clean_wx[2] = clamp(clean_wx[2], -max_cross_x, max_cross_x);
            clean_wy[1] = clamp(clean_wy[1], -max_cross_y, max_cross_y);
        }

        let norm_x = (clean_wx[1] * clean_wx[1] + clean_wx[2] * clean_wx[2] + clean_wx[3] * clean_wx[3]).sqrt();
        let norm_y = (clean_wy[1] * clean_wy[1] + clean_wy[2] * clean_wy[2] + clean_wy[3] * clean_wy[3]).sqrt();

        if norm_x < 1.0 / 35.0 && norm_x > 1e-5 {
            let scale = (1.0 / 15.0) / norm_x;
            for j in 1..4 {
                clean_wx[j] *= scale;
            }
        } else if norm_x > 1.0 / 2.5 {
            let scale = (1.0 / 2.5) / norm_x;
            for j in 1..4 {
                clean_wx[j] *= scale;
            }
        }

        if norm_y < 1.0 / 32.0 && norm_y > 1e-5 {
            let scale = (1.0 / 8.0) / norm_y;
            for j in 1..4 {
                clean_wy[j] *= scale;
            }
        } else if norm_y > 1.0 / 1.8 {
            let scale = (1.0 / 1.8) / norm_y;
            for j in 1..4 {
                clean_wy[j] *= scale;
            }
        }

        // Step 4: Independent 4-Direction Boundary Envelope Scaling (Left, Right, Up, Down)
        // Collects the de-rotated head projections at each boundary so 89% of the user's comfortable
        // boundary angle reaches 100% of the screen edge (`-1.0` and `+1.0`).
        let mut left_ratios: Vec<f32> = Vec::new();
        let mut right_ratios: Vec<f32> = Vec::new();
        let mut up_ratios: Vec<f32> = Vec::new();
        let mut down_ratios: Vec<f32> = Vec::new();

        for idx in best_lag..n {
            let s = detrended[idx];
            let tgt = detrended[idx - best_lag];
            let px = clean_wx[1] * s.yaw + clean_wx[2] * s.pitch + clean_wx[3] * s.roll;
            let py = clean_wy[1] * s.yaw + clean_wy[2] * s.pitch + clean_wy[3] * s.roll;

            if tgt.tx < -0.55 && px < -0.04 {
                left_ratios.push((tgt.tx / px).abs());
            } else if tgt.tx > 0.55 && px > 0.04 {
                right_ratios.push((tgt.tx / px).abs());
            }
            if tgt.ty < -0.55 && py < -0.04 {
                up_ratios.push((tgt.ty / py).abs());
            } else if tgt.ty > 0.55 && py > 0.04 {
                down_ratios.push((tgt.ty / py).abs());
            }
        }

        fn robust_boundary_scale(ratios: &mut [f32], comfort_boost: f32, default_scale: f32, max_scale: f32) -> f32 {
            if ratios.len() < 2 {
                return default_scale;
            }
            ratios.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            // Use 40th percentile ratio (= 60th percentile head excursion) * comfort_boost
            // so even the gentler boundary holds reliably reach 100% of the screen edge.
            let idx = ((ratios.len() - 1) as f32 * 0.45).round() as usize;
            clamp(ratios[idx] * comfort_boost, 0.55, max_scale)
        }

        self.scale_left = robust_boundary_scale(&mut left_ratios, 1.12, 1.08, 3.2);
        self.scale_right = robust_boundary_scale(&mut right_ratios, 1.12, 1.08, 3.2);
        self.scale_up = robust_boundary_scale(&mut up_ratios, 1.14, 1.12, 3.8);
        self.scale_down = robust_boundary_scale(&mut down_ratios, 1.14, 1.12, 3.8);

        // Compute R^2 fit score across lag-compensated trajectory
        let mut ss_res = 0.0f32;
        let mut ss_tot = 0.0f32;
        for idx in best_lag..n {
            let s = detrended[idx];
            let tgt = detrended[idx - best_lag];
            let raw_px = clean_wx[1] * s.yaw + clean_wx[2] * s.pitch + clean_wx[3] * s.roll;
            let raw_py = clean_wy[1] * s.yaw + clean_wy[2] * s.pitch + clean_wy[3] * s.roll;
            let px = clamp(if raw_px < 0.0 { raw_px * self.scale_left } else { raw_px * self.scale_right }, -1.0, 1.0);
            let py = clamp(if raw_py < 0.0 { raw_py * self.scale_up } else { raw_py * self.scale_down }, -1.0, 1.0);
            ss_res += (px - tgt.tx).powi(2) + (py - tgt.ty).powi(2);
            ss_tot += tgt.tx.powi(2) + tgt.ty.powi(2);
        }
        self.calib_r2 = if ss_tot > 1e-4 {
            clamp(1.0 - (ss_res / ss_tot), 0.0, 1.0)
        } else {
            0.0
        };

        self.learned_wx = clean_wx;
        self.learned_wy = clean_wy;
        self.has_learned_map = true;
        true
    }

    pub fn update(
        &mut self,
        ax: f32,
        ay: f32,
        az: f32,
        gx: f32,
        gy: f32,
        gz: f32,
        bridge_yaw: f32,
        has_bridge_yaw: bool,
        dt: f32,
    ) {
        let dt = clamp(dt, 0.005, 0.25);
        if !ax.is_finite() || !ay.is_finite() || !az.is_finite() {
            return;
        }
        let meas = normalize3([ax, ay, az]);
        if self.up.is_none() {
            if let Some(m) = meas {
                self.up = Some(m);
            } else {
                return;
            }
        }

        let g_valid = gx.is_finite() && gy.is_finite() && gz.is_finite();
        if has_bridge_yaw {
            // Bridge already estimates and subtracts 52 Hz gyro bias; do not run a second bias
            // estimator in Rust that would swallow slow user head movements!
            self.bias_calibrated = true;
            self.gyro_bias = [0.0, 0.0, 0.0];
            if self.u0.is_none() {
                if let Some(u) = self.up {
                    let (f0, r0) = build_head_basis(u);
                    self.u0 = Some(u);
                    self.f0 = f0;
                    self.r0 = r0;
                    self.yaw_zero_deg = self.yaw_deg;
                }
            }
        } else if g_valid {
            let raw_g = [gx, gy, gz];
            if !self.bias_calibrated {
                let mean = if self.still_count > 0 {
                    let n = self.still_count as f32;
                    [
                        self.still_sum[0] / n,
                        self.still_sum[1] / n,
                        self.still_sum[2] / n,
                    ]
                } else {
                    raw_g
                };
                let dev = norm3([raw_g[0] - mean[0], raw_g[1] - mean[1], raw_g[2] - mean[2]]);
                if dev <= 4.0 {
                    self.still_sum[0] += raw_g[0];
                    self.still_sum[1] += raw_g[1];
                    self.still_sum[2] += raw_g[2];
                    self.still_count += 1;
                    self.still_time += dt;
                    if self.still_time >= 0.5 && self.still_count >= 3 {
                        let n = self.still_count as f32;
                        self.gyro_bias = [
                            self.still_sum[0] / n,
                            self.still_sum[1] / n,
                            self.still_sum[2] / n,
                        ];
                        self.bias_calibrated = true;
                        if self.u0.is_none() {
                            if let Some(u) = self.up {
                                let (f0, r0) = build_head_basis(u);
                                self.u0 = Some(u);
                                self.f0 = f0;
                                self.r0 = r0;
                                self.yaw_zero_deg = 0.0;
                            }
                        }
                    }
                } else {
                    self.still_sum = raw_g;
                    self.still_count = 1;
                    self.still_time = dt;
                }
            } else {
                let w_check = [
                    raw_g[0] - self.gyro_bias[0],
                    raw_g[1] - self.gyro_bias[1],
                    raw_g[2] - self.gyro_bias[2],
                ];
                if norm3(w_check) < 1.2 {
                    let alpha_b = dt / (15.0 + dt);
                    self.gyro_bias[0] += alpha_b * (raw_g[0] - self.gyro_bias[0]);
                    self.gyro_bias[1] += alpha_b * (raw_g[1] - self.gyro_bias[1]);
                    self.gyro_bias[2] += alpha_b * (raw_g[2] - self.gyro_bias[2]);
                }
            }
        }

        let w = if g_valid {
            [
                gx - self.gyro_bias[0],
                gy - self.gyro_bias[1],
                gz - self.gyro_bias[2],
            ]
        } else {
            [0.0, 0.0, 0.0]
        };
        self.rate_deg_s = norm3(w);

        let mut u = self.up.unwrap();
        if self.rate_deg_s > 0.05 && self.bias_calibrated {
            let wx = w[0].to_radians();
            let wy = w[1].to_radians();
            let wz = w[2].to_radians();
            u = [
                u[0] + (u[1] * wz - u[2] * wy) * dt,
                u[1] + (u[2] * wx - u[0] * wz) * dt,
                u[2] + (u[0] * wy - u[1] * wx) * dt,
            ];
        }
        if let Some(m) = meas {
            // Fast gravity lock when settling (< 2.5 deg/s) eliminates vertical reverse hysteresis
            let tilt_tau = if !self.bias_calibrated {
                0.04
            } else if self.rate_deg_s < 2.5 {
                0.07
            } else {
                0.15
            };
            let alpha = tilt_tau / (tilt_tau + dt);
            u = [
                alpha * u[0] + (1.0 - alpha) * m[0],
                alpha * u[1] + (1.0 - alpha) * m[1],
                alpha * u[2] + (1.0 - alpha) * m[2],
            ];
        }
        if let Some(un) = normalize3(u) {
            u = un;
            self.up = Some(u);
        }

        self.raw_pitch = u[0].atan2((u[1] * u[1] + u[2] * u[2]).sqrt()).to_degrees();
        self.raw_roll = u[1].atan2((u[0] * u[0] + u[2] * u[2]).sqrt()).to_degrees();

        if has_bridge_yaw && bridge_yaw.is_finite() {
            // Reconstruct exact 52 Hz un-leaked gyro yaw increment by undoing bridge's 20.0s decay:
            // Holding at any screen boundary has 0.00 deg/s decay and 0.00 deg reverse overshoot!
            let delta = if let Some(prev_by) = self.prev_bridge_yaw {
                let expected_leaked = prev_by * (-dt / 20.0).exp();
                let raw_d = wrap_deg(bridge_yaw - expected_leaked);
                if raw_d.abs() > 45.0 && self.rate_deg_s > 0.1 && self.rate_deg_s < 80.0 {
                    -dot3(w, u) * dt
                } else {
                    raw_d
                }
            } else {
                bridge_yaw
            };
            self.prev_bridge_yaw = Some(bridge_yaw);
            if self.rate_deg_s >= 0.35 || delta.abs() >= 0.04 {
                self.yaw_deg = wrap_deg(self.yaw_deg + delta);
            }
        } else {
            self.prev_bridge_yaw = None;
            if self.rate_deg_s >= 0.40 {
                let yaw_rate = -dot3(w, u);
                self.yaw_deg = wrap_deg(self.yaw_deg + yaw_rate * dt);
            }
        }
        self.raw_yaw = self.yaw_deg;

        if self.u0.is_none() {
            let (f0, r0) = build_head_basis(u);
            self.u0 = Some(u);
            self.f0 = f0;
            self.r0 = r0;
            self.yaw_zero_deg = self.yaw_deg;
        }

        let u0 = self.u0.unwrap();
        let cos_u = dot3(u, u0).clamp(-1.0, 1.0);
        let sin_pitch = dot3(u, self.f0);
        let sin_roll = dot3(u, self.r0);
        let dpitch = sin_pitch.atan2(cos_u).to_degrees();
        let droll = sin_roll.atan2(cos_u).to_degrees();
        let mut dyaw = wrap_deg(self.yaw_deg - self.yaw_zero_deg);

        // Gravity-gated neutral micro-anchor: ONLY when resting still near Center posture,
        // gently bleed out long-term thermal yaw drift without EVER decaying edge holds!
        if !self.calibrating_motion
            && self.rate_deg_s < 0.8
            && dpitch.abs() < 2.2
            && droll.abs() < 2.2
            && dyaw.abs() < 3.5
        {
            self.edge_still_time += dt;
            if self.edge_still_time > 1.2 {
                let anchor_alpha = dt / (35.0 + dt);
                self.yaw_zero_deg = wrap_deg(self.yaw_zero_deg + dyaw * anchor_alpha);
                dyaw = wrap_deg(self.yaw_deg - self.yaw_zero_deg);
            }
        } else {
            self.edge_still_time = 0.0;
        }

        self.rel_pitch = dpitch;
        self.rel_roll = droll;
        self.rel_yaw = dyaw;

        let sens = self.sensitivity.max(0.25);
        let sx = if self.invert_yaw { -1.0 } else { 1.0 };
        let sy = if self.invert_pitch { -1.0 } else { 1.0 };

        let (target_x, target_y) = if self.has_learned_map {
            let wx = self.learned_wx;
            let wy = self.learned_wy;
            let raw_tx = wx[1] * dyaw + wx[2] * dpitch + wx[3] * droll;
            let raw_ty = wy[1] * dyaw + wy[2] * dpitch + wy[3] * droll;
            let reached_tx = if raw_tx < 0.0 {
                raw_tx * self.scale_left
            } else {
                raw_tx * self.scale_right
            };
            let reached_ty = if raw_ty < 0.0 {
                raw_ty * self.scale_up
            } else {
                raw_ty * self.scale_down
            };
            let scaled_x = sx * sens * reached_tx;
            let scaled_y = sy * sens * reached_ty;

            // Anti-Windup Boundary Clamping on Yaw:
            // If the user turns past the left/right screen boundary (`|scaled_x| > 1.02`),
            // shift `yaw_zero_deg` along with the excess angle so the instant they reverse
            // direction by even 0.1 deg, the cursor immediately leaves the boundary!
            if !self.calibrating_motion && scaled_x.abs() > 1.02 {
                let eff_scale_x = (if raw_tx < 0.0 { self.scale_left } else { self.scale_right })
                    * sens
                    * wx[1].abs();
                if eff_scale_x > 1e-3 {
                    let excess_norm = scaled_x.abs() - 1.02;
                    let excess_deg = (excess_norm / eff_scale_x) * dyaw.signum();
                    self.yaw_zero_deg = wrap_deg(self.yaw_zero_deg + excess_deg);
                    self.rel_yaw = wrap_deg(self.yaw_deg - self.yaw_zero_deg);
                }
            }

            (clamp(scaled_x, -1.0, 1.0), clamp(scaled_y, -1.0, 1.0))
        } else {
            let dz_yaw = self.deadzone_deg * 0.25;
            let dz_pitch = self.deadzone_deg * 0.15;
            let eff_yaw = if dyaw.abs() <= dz_yaw {
                0.0
            } else {
                dyaw.signum() * (dyaw.abs() - dz_yaw)
            };
            let eff_pitch = if dpitch.abs() <= dz_pitch {
                0.0
            } else {
                dpitch.signum() * (dpitch.abs() - dz_pitch)
            };
            let range_x = (self.yaw_range_deg / sens).max(3.0);
            let range_y = (self.pitch_range_deg / sens).max(2.0);
            let def_sy = if self.invert_pitch { 1.0 } else { -1.0 };
            let scaled_x = sx * eff_yaw / range_x;
            let scaled_y = def_sy * eff_pitch / range_y;

            if !self.calibrating_motion && scaled_x.abs() > 1.02 {
                let excess_deg = (scaled_x.abs() - 1.02) * range_x * dyaw.signum();
                self.yaw_zero_deg = wrap_deg(self.yaw_zero_deg + excess_deg);
                self.rel_yaw = wrap_deg(self.yaw_deg - self.yaw_zero_deg);
            }

            (clamp(scaled_x, -1.0, 1.0), clamp(scaled_y, -1.0, 1.0))
        };

        // 1-Euro velocity-adaptive smoothing (Casiez et al., CHI 2012):
        // Smooth when stationary (rate < 1.2 deg/s) with zero position deadzone,
        // and instant zero-lag response when moving or reversing!
        let cutoff_hz = clamp(2.2 + 1.4 * self.rate_deg_s, 2.2, 28.0);
        let rc = 1.0 / (2.0 * std::f32::consts::PI * cutoff_hz);
        let k = clamp(dt / (rc + dt), 0.18, 0.92);
        self.tilt_x += (target_x - self.tilt_x) * k;
        self.tilt_y += (target_y - self.tilt_y) * k;
        self.aim_x = clamp(0.5 + self.tilt_x * 0.5, 0.0, 1.0);
        self.aim_y = clamp(0.5 + self.tilt_y * 0.5, 0.0, 1.0);
    }

    pub fn tilt_x(&self) -> f32 {
        self.tilt_x
    }
    pub fn tilt_y(&self) -> f32 {
        self.tilt_y
    }
    pub fn aim_x(&self) -> f32 {
        self.aim_x
    }
    pub fn aim_y(&self) -> f32 {
        self.aim_y
    }
    pub fn rel_pitch_deg(&self) -> f32 {
        self.rel_pitch
    }
    pub fn rel_roll_deg(&self) -> f32 {
        self.rel_roll
    }
    pub fn rel_yaw_deg(&self) -> f32 {
        self.rel_yaw
    }
    pub fn raw_pitch_deg(&self) -> f32 {
        self.raw_pitch
    }
    pub fn raw_roll_deg(&self) -> f32 {
        self.raw_roll
    }
    pub fn raw_yaw_deg(&self) -> f32 {
        self.raw_yaw
    }
    pub fn calib_progress(&self) -> f32 {
        if self.bias_calibrated {
            1.0
        } else {
            clamp(self.still_time / 0.5, 0.0, 1.0)
        }
    }
    pub fn is_tared(&self) -> bool {
        self.u0.is_some()
    }
    pub fn has_motion_model(&self) -> bool {
        self.has_learned_map
    }
    pub fn calib_sample_count(&self) -> u32 {
        self.calib_samples.len() as u32
    }
    pub fn calib_r2_score(&self) -> f32 {
        self.calib_r2
    }
    pub fn learned_yaw_deg_for_edge(&self) -> f32 {
        let g = (self.learned_wx[1].powi(2) + self.learned_wx[2].powi(2) + self.learned_wx[3].powi(2)).sqrt();
        let mean_scale = 0.5 * (self.scale_left + self.scale_right);
        if g * mean_scale > 1e-4 {
            1.0 / (g * mean_scale)
        } else {
            self.yaw_range_deg
        }
    }
    pub fn learned_pitch_deg_for_edge(&self) -> f32 {
        let g = (self.learned_wy[1].powi(2) + self.learned_wy[2].powi(2) + self.learned_wy[3].powi(2)).sqrt();
        let mean_scale = 0.5 * (self.scale_up + self.scale_down);
        if g * mean_scale > 1e-4 {
            1.0 / (g * mean_scale)
        } else {
            self.pitch_range_deg
        }
    }
    pub fn learned_yaw_sign(&self) -> f32 {
        if self.learned_wx[1] >= 0.0 { 1.0 } else { -1.0 }
    }
    pub fn learned_pitch_sign(&self) -> f32 {
        if self.learned_wy[2] >= 0.0 { 1.0 } else { -1.0 }
    }
}

const BLINK_DIM: usize = 6;
const ROLLING_EEG_SAMPLES: usize = 28;

#[derive(Clone, Copy)]
struct BlinkWindow {
    feat: [f32; BLINK_DIM],
    ptp_tp9: f32,
    ptp_af7: f32,
    ptp_af8: f32,
    ptp_tp10: f32,
    eff_amp: f32,
    corr: f32,
    prominence: f32,
}

fn detrend_linear(xs: &[f32]) -> Vec<f32> {
    let n = xs.len();
    if n < 2 {
        return xs.to_vec();
    }
    let nf = n as f32;
    let mean_x = (nf - 1.0) * 0.5;
    let mean_y: f32 = xs.iter().copied().filter(|v| v.is_finite()).sum::<f32>() / nf;
    let mut num = 0.0f32;
    let mut den = 0.0f32;
    for (i, &y) in xs.iter().enumerate() {
        if y.is_finite() {
            let dx = i as f32 - mean_x;
            num += dx * (y - mean_y);
            den += dx * dx;
        }
    }
    let slope = if den > 1e-6 { num / den } else { 0.0 };
    let detrended: Vec<f32> = xs
        .iter()
        .enumerate()
        .map(|(i, &y)| {
            if y.is_finite() {
                y - (mean_y + slope * (i as f32 - mean_x))
            } else {
                0.0
            }
        })
        .collect();

    if detrended.len() < 5 {
        return detrended;
    }
    // 3-tap low-pass filter [0.25, 0.50, 0.25] attenuates 50/60Hz mains hum & high-freq EMG
    // while preserving the slow (~200ms) corneal-retinal blink wave.
    let mut smoothed = Vec::with_capacity(detrended.len());
    smoothed.push(0.65 * detrended[0] + 0.35 * detrended[1]);
    for i in 1..(detrended.len() - 1) {
        smoothed.push(0.25 * detrended[i - 1] + 0.50 * detrended[i] + 0.25 * detrended[i + 1]);
    }
    let last = detrended.len() - 1;
    smoothed.push(0.35 * detrended[last - 1] + 0.65 * detrended[last]);
    smoothed
}

fn ptp_and_prominence(raw: &[f32]) -> (f32, f32) {
    if raw.len() < 2 {
        return (0.0, 0.0);
    }
    let xs = detrend_linear(raw);
    let mut lo = f32::INFINITY;
    let mut hi = f32::NEG_INFINITY;
    let mut sum = 0.0f32;
    let mut count = 0usize;
    for &v in &xs {
        if v.is_finite() {
            if v < lo {
                lo = v;
            }
            if v > hi {
                hi = v;
            }
            sum += v;
            count += 1;
        }
    }
    if count < 2 {
        return (0.0, 0.0);
    }
    let mean = sum / count as f32;
    let mut mad_sum = 0.0f32;
    let mut peak_dev = 0.0f32;
    for &v in &xs {
        if v.is_finite() {
            let d = (v - mean).abs();
            mad_sum += d;
            if d > peak_dev {
                peak_dev = d;
            }
        }
    }
    let mad = (mad_sum / count as f32).max(1.0);
    (hi - lo, peak_dev / mad)
}

fn pearson_corr(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len());
    if n < 3 {
        return 0.0;
    }
    let da_vec = detrend_linear(&a[a.len() - n..]);
    let db_vec = detrend_linear(&b[b.len() - n..]);
    let mut num = 0.0f32;
    let mut da2 = 0.0f32;
    let mut db2 = 0.0f32;
    for i in 0..n {
        let da = da_vec[i];
        let db = db_vec[i];
        num += da * db;
        da2 += da * da;
        db2 += db * db;
    }
    let den = (da2 * db2).sqrt();
    if den > 1e-6 {
        clamp(num / den, -1.0, 1.0)
    } else {
        0.0
    }
}

fn compute_eff_amp(
    ptp_tp9: f32,
    ptp_af7: f32,
    ptp_af8: f32,
    ptp_tp10: f32,
    noise_floor: [f32; 4],
) -> f32 {
    let exc_tp9 = (ptp_tp9 - noise_floor[0] * 0.75).max(0.0);
    let exc_af7 = (ptp_af7 - noise_floor[1] * 0.50).max(0.0);
    let exc_af8 = (ptp_af8 - noise_floor[2] * 0.50).max(0.0);
    let exc_tp10 = (ptp_tp10 - noise_floor[3] * 0.75).max(0.0);
    let max_frontal = exc_af7.max(exc_af8);
    let sync_temporal = exc_tp9.min(exc_tp10);
    max_frontal.max(sync_temporal * 0.85)
}

fn extract_blink_window(
    tp9: &[f32],
    af7: &[f32],
    af8: &[f32],
    tp10: &[f32],
    noise_floor: [f32; 4],
) -> BlinkWindow {
    let (ptp_tp9, prom_9) = ptp_and_prominence(tp9);
    let (ptp_af7, prom_7) = ptp_and_prominence(af7);
    let (ptp_af8, prom_8) = ptp_and_prominence(af8);
    let (ptp_tp10, prom_10) = ptp_and_prominence(tp10);

    // On Muse 2, the reference electrode Fpz sits in the center of the forehead above the eyes.
    // Eye blinks shift Fpz, producing synchronous spikes across TP9, TP10, and AF7/AF8.
    let corr_frontal = pearson_corr(af7, af8);
    let corr_temporal = pearson_corr(tp9, tp10);
    let corr_cross = pearson_corr(af8, tp10).max(pearson_corr(af7, tp9));
    let corr = corr_frontal.max(corr_temporal).max(corr_cross);

    let eff_amp = compute_eff_amp(ptp_tp9, ptp_af7, ptp_af8, ptp_tp10, noise_floor);
    let prominence = prom_7.max(prom_8).max(prom_9.min(prom_10));

    let feat = [
        (ptp_tp9 + 1.0).ln(),
        (ptp_af7 + 1.0).ln(),
        (ptp_af8 + 1.0).ln(),
        (ptp_tp10 + 1.0).ln(),
        corr,
        prominence,
    ];

    BlinkWindow {
        feat,
        ptp_tp9,
        ptp_af7,
        ptp_af8,
        ptp_tp10,
        eff_amp,
        corr,
        prominence,
    }
}

fn push_rolling(buf: &mut Vec<f32>, incoming: &[f32], max_len: usize) {
    for &v in incoming {
        if v.is_finite() {
            buf.push(v);
        }
    }
    if buf.len() > max_len {
        let drop_n = buf.len() - max_len;
        buf.drain(0..drop_n);
    }
}

#[wasm_bindgen]
pub struct RustBlinkClassifier {
    buf_tp9: Vec<f32>,
    buf_af7: Vec<f32>,
    buf_af8: Vec<f32>,
    buf_tp10: Vec<f32>,
    noise_floor: [f32; 4],
    recent: Vec<BlinkWindow>,
    positives: Vec<BlinkWindow>,
    negatives: Vec<BlinkWindow>,
    weights: [f32; BLINK_DIM],
    bias: f32,
    feat_mean: [f32; BLINK_DIM],
    feat_std: [f32; BLINK_DIM],
    trained: bool,
    accuracy: f32,
    learned_threshold_uv: f32,
    learned_af7_uv: f32,
    learned_af8_uv: f32,
    capture_timer: f32,
    cooldown: f32,
    rearmed: bool,
    quiet_timer: f32,
    last_prob: f32,
    last_tp9_ptp: f32,
    last_af7_ptp: f32,
    last_af8_ptp: f32,
    last_tp10_ptp: f32,
    last_eff_amp: f32,
    last_corr: f32,
    total_detected: u32,
}

#[wasm_bindgen]
impl RustBlinkClassifier {
    #[wasm_bindgen(constructor)]
    pub fn new() -> RustBlinkClassifier {
        RustBlinkClassifier {
            buf_tp9: Vec::with_capacity(ROLLING_EEG_SAMPLES),
            buf_af7: Vec::with_capacity(ROLLING_EEG_SAMPLES),
            buf_af8: Vec::with_capacity(ROLLING_EEG_SAMPLES),
            buf_tp10: Vec::with_capacity(ROLLING_EEG_SAMPLES),
            noise_floor: [45.0, 30.0, 30.0, 45.0],
            recent: Vec::with_capacity(30),
            positives: Vec::new(),
            negatives: Vec::new(),
            weights: [0.45, 0.45, 0.55, 0.45, 0.65, 0.45],
            bias: -0.1,
            feat_mean: [4.2, 4.0, 4.2, 4.2, 0.2, 2.0],
            feat_std: [1.0; BLINK_DIM],
            trained: false,
            accuracy: 0.0,
            learned_threshold_uv: 175.0,
            learned_af7_uv: 0.0,
            learned_af8_uv: 0.0,
            capture_timer: 0.0,
            cooldown: 0.0,
            rearmed: true,
            quiet_timer: 0.0,
            last_prob: 0.0,
            last_tp9_ptp: 0.0,
            last_af7_ptp: 0.0,
            last_af8_ptp: 0.0,
            last_tp10_ptp: 0.0,
            last_eff_amp: 0.0,
            last_corr: 0.0,
            total_detected: 0,
        }
    }

    pub fn reset_model(&mut self) {
        self.positives.clear();
        self.negatives.clear();
        self.noise_floor = [45.0, 30.0, 30.0, 45.0];
        self.trained = false;
        self.accuracy = 0.0;
        self.learned_threshold_uv = 175.0;
        self.learned_af7_uv = 0.0;
        self.learned_af8_uv = 0.0;
        self.capture_timer = 0.0;
        self.last_prob = 0.0;
    }

    /// Searches the last ~2.5s of EEG windows for the strongest blink peak if the user blinked
    /// before clicking, or arms a 2.5s capture window if they click first and then blink.
    pub fn record_blink_button(&mut self) -> bool {
        if let Some(best) = self.best_recent_candidate(85.0) {
            self.add_positive(best);
            self.capture_timer = 0.0;
            return true;
        }
        self.capture_timer = 2.5;
        false
    }

    /// Arms a flash-cue capture window (e.g., 1.2s) right when the visual cue flashes on screen.
    pub fn arm_flash_window(&mut self, seconds: f32) {
        self.capture_timer = clamp(seconds, 0.3, 3.5);
    }

    /// Records the latest EEG window as a non-blink negative sample (used during Hold Still
    /// and Move Head Around Without Blinking calibration stages to cancel noise & hum).
    pub fn record_baseline_button(&mut self) -> bool {
        if let Some(&last) = self.recent.last() {
            if last.ptp_af7 < 1200.0 && last.ptp_af8 < 1200.0 {
                self.add_negative(last);
                return true;
            }
        }
        false
    }

    fn best_recent_candidate(&self, min_uv: f32) -> Option<BlinkWindow> {
        let mut best: Option<BlinkWindow> = None;
        for w in &self.recent {
            if w.eff_amp >= min_uv && w.eff_amp <= 2200.0 {
                if best.map_or(true, |b| w.eff_amp > b.eff_amp) {
                    best = Some(*w);
                }
            }
        }
        best
    }

    fn update_noise_floor_from_negatives(&mut self) {
        if self.negatives.is_empty() {
            return;
        }
        let n = self.negatives.len();
        let idx75 = ((n - 1) * 3) / 4;
        let mut v9: Vec<f32> = self.negatives.iter().map(|w| w.ptp_tp9).collect();
        let mut v7: Vec<f32> = self.negatives.iter().map(|w| w.ptp_af7).collect();
        let mut v8: Vec<f32> = self.negatives.iter().map(|w| w.ptp_af8).collect();
        let mut v10: Vec<f32> = self.negatives.iter().map(|w| w.ptp_tp10).collect();
        v9.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        v7.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        v8.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        v10.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        self.noise_floor = [
            clamp(v9[idx75], 20.0, 420.0),
            clamp(v7[idx75], 15.0, 260.0),
            clamp(v8[idx75], 15.0, 260.0),
            clamp(v10[idx75], 20.0, 420.0),
        ];

        for w in &mut self.negatives {
            w.eff_amp = compute_eff_amp(w.ptp_tp9, w.ptp_af7, w.ptp_af8, w.ptp_tp10, self.noise_floor);
        }
        for w in &mut self.positives {
            w.eff_amp = compute_eff_amp(w.ptp_tp9, w.ptp_af7, w.ptp_af8, w.ptp_tp10, self.noise_floor);
        }
    }

    fn add_positive(&mut self, w: BlinkWindow) {
        self.positives.push(w);
        if self.positives.len() > 80 {
            self.positives.remove(0);
        }
        if self.negatives.len() < 4 {
            for r in &self.recent {
                if r.eff_amp < w.eff_amp * 0.50 {
                    self.negatives.push(*r);
                }
            }
        }
        if self.negatives.len() < 3 {
            let synth = BlinkWindow {
                feat: [3.4, 3.2, 3.4, 3.4, 0.0, 1.4],
                ptp_tp9: 35.0,
                ptp_af7: 25.0,
                ptp_af8: 30.0,
                ptp_tp10: 35.0,
                eff_amp: 20.0,
                corr: 0.0,
                prominence: 1.4,
            };
            self.negatives.push(synth);
        }
        self.train();
    }

    fn add_negative(&mut self, w: BlinkWindow) {
        self.negatives.push(w);
        if self.negatives.len() > 140 {
            self.negatives.remove(0);
        }
        self.update_noise_floor_from_negatives();
        if !self.positives.is_empty() {
            self.train();
        }
    }

    fn train(&mut self) {
        if self.positives.is_empty() {
            return;
        }
        self.update_noise_floor_from_negatives();
        let n_pos = self.positives.len();
        let n_neg = self.negatives.len();

        let mut af7_sum = 0.0f32;
        let mut af8_sum = 0.0f32;
        let mut eff_sum = 0.0f32;
        for p in &self.positives {
            af7_sum += p.ptp_af7;
            af8_sum += p.ptp_af8;
            eff_sum += p.eff_amp;
        }
        self.learned_af7_uv = af7_sum / n_pos as f32;
        self.learned_af8_uv = af8_sum / n_pos as f32;
        let pos_mean_eff = eff_sum / n_pos as f32;

        let (neg_mean_eff, neg_p95_eff) = if n_neg > 0 {
            let mean = self.negatives.iter().map(|w| w.eff_amp).sum::<f32>() / n_neg as f32;
            let mut neg_effs: Vec<f32> = self.negatives.iter().map(|w| w.eff_amp).collect();
            neg_effs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let idx95 = ((n_neg - 1) * 95) / 100;
            (mean, neg_effs[idx95])
        } else {
            (25.0, 45.0)
        };

        let mid_thresh = neg_mean_eff + 0.52 * (pos_mean_eff - neg_mean_eff);
        let safe_floor = neg_p95_eff * 1.25 + 18.0;
        let max_allowed = (pos_mean_eff * 0.85).max(95.0);
        self.learned_threshold_uv = clamp(
            mid_thresh.max(safe_floor).min(max_allowed),
            85.0,
            700.0,
        );

        let total = (n_pos + n_neg) as f32;
        if n_neg == 0 {
            return;
        }

        for j in 0..BLINK_DIM {
            let sum: f32 = self
                .positives
                .iter()
                .chain(self.negatives.iter())
                .map(|w| w.feat[j])
                .sum();
            let mean = sum / total;
            let var: f32 = self
                .positives
                .iter()
                .chain(self.negatives.iter())
                .map(|w| {
                    let d = w.feat[j] - mean;
                    d * d
                })
                .sum::<f32>()
                / total;
            self.feat_mean[j] = mean;
            self.feat_std[j] = var.sqrt().max(0.15);
        }

        let mut w = [0.0f32; BLINK_DIM];
        let mut b = 0.0f32;
        // Weight negatives slightly higher (1.25x) so false blinks from head movement or hum are heavily penalized
        let w_pos = total / (2.0 * n_pos as f32);
        let w_neg = (total / (2.0 * n_neg as f32)) * 1.25;
        let lr = 0.12f32;
        let l2 = 0.04f32;

        for _epoch in 0..140 {
            let mut gw = [0.0f32; BLINK_DIM];
            let mut gb = 0.0f32;

            for sample in &self.positives {
                let mut z = b;
                let mut x = [0.0f32; BLINK_DIM];
                for j in 0..BLINK_DIM {
                    x[j] = (sample.feat[j] - self.feat_mean[j]) / self.feat_std[j];
                    z += w[j] * x[j];
                }
                let p = 1.0 / (1.0 + (-z.clamp(-15.0, 15.0)).exp());
                let err = (p - 1.0) * w_pos;
                for j in 0..BLINK_DIM {
                    gw[j] += err * x[j];
                }
                gb += err;
            }

            for sample in &self.negatives {
                let mut z = b;
                let mut x = [0.0f32; BLINK_DIM];
                for j in 0..BLINK_DIM {
                    x[j] = (sample.feat[j] - self.feat_mean[j]) / self.feat_std[j];
                    z += w[j] * x[j];
                }
                let p = 1.0 / (1.0 + (-z.clamp(-15.0, 15.0)).exp());
                let err = p * w_neg;
                for j in 0..BLINK_DIM {
                    gw[j] += err * x[j];
                }
                gb += err;
            }

            for j in 0..BLINK_DIM {
                w[j] -= lr * (gw[j] / total + l2 * w[j]);
            }
            b -= lr * (gb / total);
        }

        self.weights = w;
        self.bias = b;
        self.trained = true;

        let mut correct = 0usize;
        for sample in &self.positives {
            if self.predict_window(sample) >= 0.55 {
                correct += 1;
            }
        }
        for sample in &self.negatives {
            if self.predict_window(sample) < 0.55 {
                correct += 1;
            }
        }
        self.accuracy = correct as f32 / (n_pos + n_neg) as f32;
    }

    fn predict_window(&self, w: &BlinkWindow) -> f32 {
        if w.eff_amp > 2200.0 || w.eff_amp < 45.0 {
            return 0.0;
        }
        if w.corr < 0.20 || w.prominence < 1.65 {
            return 0.02;
        }
        if self.trained {
            let mut z = self.bias;
            for j in 0..BLINK_DIM {
                let x = (w.feat[j] - self.feat_mean[j]) / self.feat_std[j];
                z += self.weights[j] * x;
            }
            let lr_prob = 1.0 / (1.0 + (-z.clamp(-15.0, 15.0)).exp());
            let thresh_ratio = (w.eff_amp - self.learned_threshold_uv) / 38.0;
            let amp_prob = 1.0 / (1.0 + (-thresh_ratio.clamp(-15.0, 15.0)).exp());
            clamp(0.55 * lr_prob + 0.45 * amp_prob, 0.0, 1.0)
        } else {
            if w.corr < 0.35 || w.prominence < 1.95 {
                return 0.04;
            }
            let ratio = (w.eff_amp - self.learned_threshold_uv) / 40.0;
            let base_p = 1.0 / (1.0 + (-ratio.clamp(-12.0, 12.0)).exp());
            clamp(base_p, 0.0, 1.0)
        }
    }

    pub fn ingest_frame(
        &mut self,
        tp9: &[f32],
        af7: &[f32],
        af8: &[f32],
        tp10: &[f32],
        motion: f32,
        dt: f32,
    ) -> bool {
        let dt = clamp(dt, 0.01, 0.5);
        self.cooldown = (self.cooldown - dt).max(0.0);

        if af7.is_empty() && af8.is_empty() && tp9.is_empty() && tp10.is_empty() {
            return false;
        }

        push_rolling(&mut self.buf_tp9, tp9, ROLLING_EEG_SAMPLES);
        push_rolling(&mut self.buf_af7, af7, ROLLING_EEG_SAMPLES);
        push_rolling(&mut self.buf_af8, af8, ROLLING_EEG_SAMPLES);
        push_rolling(&mut self.buf_tp10, tp10, ROLLING_EEG_SAMPLES);

        if self.buf_af7.len() < 4 && self.buf_af8.len() < 4 {
            return false;
        }

        // Before explicit negative calibration, gently adapt per-channel noise floor on non-blink frames
        if self.negatives.len() < 8 && self.capture_timer <= 0.0 {
            let (raw9, _) = ptp_and_prominence(&self.buf_tp9);
            let (raw7, _) = ptp_and_prominence(&self.buf_af7);
            let (raw8, _) = ptp_and_prominence(&self.buf_af8);
            let (raw10, _) = ptp_and_prominence(&self.buf_tp10);
            if raw7 < 130.0 && raw8 < 130.0 {
                let alpha = clamp(dt * 0.6, 0.01, 0.15);
                self.noise_floor[0] = clamp(self.noise_floor[0] * (1.0 - alpha) + raw9 * alpha, 20.0, 380.0);
                self.noise_floor[1] = clamp(self.noise_floor[1] * (1.0 - alpha) + raw7 * alpha, 15.0, 180.0);
                self.noise_floor[2] = clamp(self.noise_floor[2] * (1.0 - alpha) + raw8 * alpha, 15.0, 180.0);
                self.noise_floor[3] = clamp(self.noise_floor[3] * (1.0 - alpha) + raw10 * alpha, 20.0, 380.0);
            }
        }

        let win = extract_blink_window(
            &self.buf_tp9,
            &self.buf_af7,
            &self.buf_af8,
            &self.buf_tp10,
            self.noise_floor,
        );
        self.last_tp9_ptp = win.ptp_tp9;
        self.last_af7_ptp = win.ptp_af7;
        self.last_af8_ptp = win.ptp_af8;
        self.last_tp10_ptp = win.ptp_tp10;
        self.last_eff_amp = win.eff_amp;
        self.last_corr = win.corr;

        self.recent.push(win);
        if self.recent.len() > 25 {
            self.recent.remove(0);
        }

        if self.capture_timer > 0.0 {
            self.capture_timer = (self.capture_timer - dt).max(0.0);
            if self.capture_timer == 0.0 {
                if let Some(best) = self.best_recent_candidate(75.0) {
                    self.add_positive(best);
                }
            } else if win.eff_amp >= 120.0 && win.eff_amp <= 2200.0 && motion < 0.45 {
                self.add_positive(win);
                self.capture_timer = 0.0;
            }
        } else if motion < 0.18 && win.eff_amp > 5.0 && win.eff_amp < 65.0 {
            self.quiet_timer += dt;
            if self.quiet_timer >= 0.8 {
                self.quiet_timer = 0.0;
                self.add_negative(win);
            }
        }

        if win.eff_amp < self.learned_threshold_uv * 0.62 {
            self.rearmed = true;
        }

        let prob = if motion > 0.38 {
            0.0
        } else {
            self.predict_window(&win)
        };
        self.last_prob = prob;

        let trigger_thresh = if self.trained { 0.58 } else { 0.68 };
        let amp_gate = win.eff_amp >= self.learned_threshold_uv * 0.85;
        if prob >= trigger_thresh && amp_gate && self.rearmed && self.cooldown <= 0.0 {
            self.rearmed = false;
            self.cooldown = 0.48;
            self.total_detected += 1;
            return true;
        }
        false
    }

    pub fn is_trained(&self) -> bool {
        self.trained
    }
    pub fn is_capturing(&self) -> bool {
        self.capture_timer > 0.0
    }
    pub fn blink_samples(&self) -> u32 {
        self.positives.len() as u32
    }
    pub fn baseline_samples(&self) -> u32 {
        self.negatives.len() as u32
    }
    pub fn accuracy(&self) -> f32 {
        self.accuracy
    }
    pub fn last_probability(&self) -> f32 {
        self.last_prob
    }
    pub fn learned_threshold_uv(&self) -> f32 {
        self.learned_threshold_uv
    }
    pub fn learned_af7_uv(&self) -> f32 {
        self.learned_af7_uv
    }
    pub fn learned_af8_uv(&self) -> f32 {
        self.learned_af8_uv
    }
    pub fn last_tp9_ptp(&self) -> f32 {
        self.last_tp9_ptp
    }
    pub fn last_af7_ptp(&self) -> f32 {
        self.last_af7_ptp
    }
    pub fn last_af8_ptp(&self) -> f32 {
        self.last_af8_ptp
    }
    pub fn last_tp10_ptp(&self) -> f32 {
        self.last_tp10_ptp
    }
    pub fn last_eff_amp(&self) -> f32 {
        self.last_eff_amp
    }
    pub fn last_corr(&self) -> f32 {
        self.last_corr
    }
    pub fn total_detected(&self) -> u32 {
        self.total_detected
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imu_tare_works_from_any_starting_pose_and_never_pins_top() {
        let mut imu = RustImuTracker::new();
        for _ in 0..10 {
            imu.update(0.25, 0.30, 0.92, 0.1, -0.2, 0.05, 0.0, true, 0.1);
        }
        assert!(imu.is_tared());
        assert!((imu.aim_x() - 0.5).abs() < 0.03);
        assert!((imu.aim_y() - 0.5).abs() < 0.03);

        for _ in 0..8 {
            imu.update(-0.10, 0.30, 0.95, 0.0, 15.0, 0.0, 0.0, true, 0.1);
        }
        assert!(imu.aim_y() > 0.58, "aim_y={} should move down", imu.aim_y());

        for _ in 0..8 {
            imu.update(0.55, 0.30, 0.78, 0.0, -15.0, 0.0, 0.0, true, 0.1);
        }
        assert!(imu.aim_y() < 0.42, "aim_y={} should move up", imu.aim_y());

        imu.tare();
        imu.update(0.55, 0.30, 0.78, 0.1, -0.2, 0.05, 0.0, true, 0.1);
        assert!((imu.aim_x() - 0.5).abs() < 0.03);
        assert!((imu.aim_y() - 0.5).abs() < 0.03);
    }

    #[test]
    fn blink_classifier_learns_asymmetric_and_fpz_reference_blinks_and_rejects_hum() {
        let mut clf = RustBlinkClassifier::new();
        // Simulate noisy TP9/TP10 ear hum (high-freq alternating ±120uV) + quiet AF7/AF8 during non-blink hold
        let hum_tp9 = [-110.0, 115.0, -105.0, 110.0, -115.0, 108.0, -112.0, 105.0];
        let hum_tp10 = [105.0, -110.0, 115.0, -108.0, 112.0, -105.0, 110.0, -115.0];
        let quiet_af = [-15.0, 12.0, -10.0, 14.0, -12.0, 10.0, -11.0, 9.0];
        for _ in 0..10 {
            let fired = clf.ingest_frame(&hum_tp9, &quiet_af, &quiet_af, &hum_tp10, 0.02, 0.1);
            assert!(!fired, "Continuous ear hum must never trigger a false blink");
            clf.record_baseline_button();
        }
        // Simulate the exact blink pattern from the user's Muse 2 screenshot:
        // TP9 & TP10 have large slow 380uV spikes, AF8 has 180uV spike, AF7 has tiny 30uV wave
        let blink_tp9 = [-140.0, -90.0, 50.0, 240.0, 230.0, 90.0, -80.0, -110.0];
        let blink_af7 = [-12.0, -6.0, 5.0, 18.0, 16.0, 4.0, -8.0, -10.0];
        let blink_af8 = [-65.0, -40.0, 25.0, 115.0, 110.0, 45.0, -45.0, -55.0];
        let blink_tp10 = [-135.0, -85.0, 45.0, 235.0, 225.0, 85.0, -75.0, -105.0];
        clf.ingest_frame(&blink_tp9, &blink_af7, &blink_af8, &blink_tp10, 0.03, 0.1);

        let committed = clf.record_blink_button();
        assert!(committed);
        assert!(clf.is_trained());
        assert_eq!(clf.blink_samples(), 1);

        for _ in 0..6 {
            let fired = clf.ingest_frame(&hum_tp9, &quiet_af, &quiet_af, &hum_tp10, 0.02, 0.1);
            assert!(!fired, "Post-training ear hum must not fire false blinks");
        }
        let detected = clf.ingest_frame(&blink_tp9, &blink_af7, &blink_af8, &blink_tp10, 0.03, 0.1);
        assert!(
            detected,
            "Trained Rust classifier should detect Fpz-referenced Muse 2 blink (prob={}, eff={}, thr={})",
            clf.last_probability(),
            clf.last_eff_amp(),
            clf.learned_threshold_uv()
        );
    }

    #[test]
    fn imu_discrete_edge_dots_reach_full_top_and_bottom_even_with_small_vertical_pitch() {
        let mut imu = RustImuTracker::new();
        for _ in 0..8 {
            imu.update(0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, true, 0.1);
        }
        imu.begin_motion_calibration();

        // Simulate discrete full-edge dots where user only tilts head +4.5 deg up for Top Edge (0.04)
        // and -6.5 deg down for Bottom Edge (0.96), and ±14 deg for Left/Right edges
        let targets = [
            (0.50, 0.50, 0.0f32, 0.0f32),
            (0.50, 0.04, 0.0, 4.5),
            (0.50, 0.96, 0.0, -6.5),
            (0.04, 0.50, -14.0, 0.0),
            (0.96, 0.50, 14.0, 0.0),
            (0.06, 0.06, -13.0, 4.2),
            (0.94, 0.06, 13.0, 4.2),
            (0.06, 0.94, -13.0, -6.0),
            (0.94, 0.94, 13.0, -6.0),
        ];
        for &(dx, dy, yaw, pitch) in &targets {
            let ax = pitch.to_radians().sin();
            let az = pitch.to_radians().cos();
            for _ in 0..4 {
                imu.update(ax, 0.0, az, 0.0, 0.0, 0.0, yaw, true, 0.08);
            }
            imu.add_calib_point(dx, dy);
            imu.add_calib_point(dx, dy);
        }

        assert!(imu.finish_motion_calibration());
        assert!(imu.has_motion_model());
        assert!(imu.calib_r2_score() > 0.85, "r2={}", imu.calib_r2_score());

        // Verify looking Up (+4.5 deg) reaches near full top edge (aim_y < 0.08) and NEVER drifts back!
        let ax_up = 4.5f32.to_radians().sin();
        let az_up = 4.5f32.to_radians().cos();
        for _ in 0..25 {
            imu.update(ax_up, 0.0, az_up, 0.0, 0.0, 0.0, 0.0, true, 0.1);
        }
        assert!(imu.aim_y() < 0.08, "aim_y={} should reach top edge and stay there", imu.aim_y());

        // Verify looking Down (-6.5 deg) reaches near full bottom edge (aim_y > 0.92)
        let ax_dn = (-6.5f32).to_radians().sin();
        let az_dn = (-6.5f32).to_radians().cos();
        for _ in 0..25 {
            imu.update(ax_dn, 0.0, az_dn, 0.0, 0.0, 0.0, 0.0, true, 0.1);
        }
        assert!(imu.aim_y() > 0.92, "aim_y={} should reach bottom edge", imu.aim_y());
    }

    #[test]
    fn imu_hybrid_pursuit_boundary_and_reverse_has_zero_overshoot_and_anti_windup() {
        let mut imu = RustImuTracker::new();
        let dt = 0.1f32;
        let mut bridge_yaw = 0.0f32;
        for _ in 0..8 {
            imu.update(0.0, 0.0, 1.0, 0.0, 0.0, 0.0, bridge_yaw, true, dt);
        }
        imu.begin_motion_calibration();

        // Helper that simulates bridge_yaw with 20s exponential leak + physical yaw/pitch change
        let mut feed_pose = |imu: &mut RustImuTracker, true_yaw: f32, prev_true_yaw: &mut f32, pitch_deg: f32, dot_x: f32, dot_y: f32, rec: bool| {
            let dyaw = true_yaw - *prev_true_yaw;
            *prev_true_yaw = true_yaw;
            let gz = -dyaw / dt;
            let wy = -pitch_deg.to_radians() / dt * 0.1;
            bridge_yaw = (bridge_yaw + dyaw) * (-dt / 20.0).exp();
            let ax = pitch_deg.to_radians().sin();
            let az = pitch_deg.to_radians().cos();
            imu.update(ax, 0.0, az, 0.0, wy, gz, bridge_yaw, true, dt);
            if rec {
                imu.add_calib_point(dot_x, dot_y);
            }
        };

        let mut prev_yaw = 0.0f32;
        // 1. Center Hold
        for _ in 0..10 {
            feed_pose(&mut imu, 0.0, &mut prev_yaw, 0.0, 0.50, 0.50, true);
        }
        // 2. Smooth pursuit Center -> Left (-14 deg), Hold at Left (0.04, 0.50), Reverse -> Center
        for k in 1..=10 {
            let frac = k as f32 / 10.0;
            feed_pose(&mut imu, -14.0 * frac, &mut prev_yaw, 0.0, 0.50 - 0.46 * frac, 0.50, true);
        }
        for _ in 0..10 {
            feed_pose(&mut imu, -14.0, &mut prev_yaw, 0.0, 0.04, 0.50, true);
        }
        for k in 1..=10 {
            let frac = 1.0 - (k as f32 / 10.0);
            feed_pose(&mut imu, -14.0 * frac, &mut prev_yaw, 0.0, 0.50 - 0.46 * frac, 0.50, true);
        }
        // Center checkpoint
        for _ in 0..5 {
            feed_pose(&mut imu, 0.0, &mut prev_yaw, 0.0, 0.50, 0.50, true);
        }
        // 3. Smooth pursuit Center -> Right (+14 deg), Hold at Right (0.96, 0.50), Reverse -> Center
        for k in 1..=10 {
            let frac = k as f32 / 10.0;
            feed_pose(&mut imu, 14.0 * frac, &mut prev_yaw, 0.0, 0.50 + 0.46 * frac, 0.50, true);
        }
        for _ in 0..10 {
            feed_pose(&mut imu, 14.0, &mut prev_yaw, 0.0, 0.96, 0.50, true);
        }
        for k in 1..=10 {
            let frac = 1.0 - (k as f32 / 10.0);
            feed_pose(&mut imu, 14.0 * frac, &mut prev_yaw, 0.0, 0.50 + 0.46 * frac, 0.50, true);
        }
        // Center checkpoint
        for _ in 0..5 {
            feed_pose(&mut imu, 0.0, &mut prev_yaw, 0.0, 0.50, 0.50, true);
        }
        // 4. Vertical Center -> Top (+5 deg), Hold Top (0.50, 0.04), Reverse -> Bottom (-7 deg), Hold Bottom (0.50, 0.96), Reverse -> Center
        for k in 1..=8 {
            let frac = k as f32 / 8.0;
            feed_pose(&mut imu, 0.0, &mut prev_yaw, 5.0 * frac, 0.50, 0.50 - 0.46 * frac, true);
        }
        for _ in 0..10 {
            feed_pose(&mut imu, 0.0, &mut prev_yaw, 5.0, 0.50, 0.04, true);
        }
        for k in 1..=8 {
            let frac = 1.0 - (k as f32 / 8.0);
            feed_pose(&mut imu, 0.0, &mut prev_yaw, 5.0 * frac, 0.50, 0.50 - 0.46 * frac, true);
        }
        for _ in 0..4 {
            feed_pose(&mut imu, 0.0, &mut prev_yaw, 0.0, 0.50, 0.50, true);
        }
        for k in 1..=8 {
            let frac = k as f32 / 8.0;
            feed_pose(&mut imu, 0.0, &mut prev_yaw, -7.0 * frac, 0.50, 0.50 + 0.46 * frac, true);
        }
        for _ in 0..10 {
            feed_pose(&mut imu, 0.0, &mut prev_yaw, -7.0, 0.50, 0.96, true);
        }
        for k in 1..=8 {
            let frac = 1.0 - (k as f32 / 8.0);
            feed_pose(&mut imu, 0.0, &mut prev_yaw, -7.0 * frac, 0.50, 0.50 + 0.46 * frac, true);
        }
        for _ in 0..6 {
            feed_pose(&mut imu, 0.0, &mut prev_yaw, 0.0, 0.50, 0.50, true);
        }

        assert!(imu.finish_motion_calibration());
        assert!(imu.calib_r2_score() > 0.85, "r2={}", imu.calib_r2_score());

        // Test 1: Hold at Left Boundary (-14 deg) for 6 seconds (60 frames) while bridge_yaw leaks,
        // then reverse back to 0.0 deg -> must land back at Center (0.50 +- 0.04) with NO overshoot!
        for _ in 0..60 {
            feed_pose(&mut imu, -14.0, &mut prev_yaw, 0.0, 0.0, 0.5, false);
        }
        assert!(imu.aim_x() < 0.05, "Left boundary hold aim_x={}", imu.aim_x());
        for _ in 0..10 {
            feed_pose(&mut imu, 0.0, &mut prev_yaw, 0.0, 0.5, 0.5, false);
        }
        assert!(
            (imu.aim_x() - 0.50).abs() < 0.05,
            "Reverse from Left boundary back to physical center must not overshoot! aim_x={}",
            imu.aim_x()
        );

        // Test 2: Anti-windup when turning far past Right Boundary (+24 deg vs +14 deg edge)
        // Reversing by just -3 deg (+24 -> +21 deg) must immediately move cursor off 1.0!
        for _ in 0..10 {
            feed_pose(&mut imu, 24.0, &mut prev_yaw, 0.0, 1.0, 0.5, false);
        }
        assert!(imu.aim_x() > 0.98);
        for _ in 0..6 {
            feed_pose(&mut imu, 20.5, &mut prev_yaw, 0.0, 0.8, 0.5, false);
        }
        assert!(
            imu.aim_x() < 0.95,
            "Anti-windup must let immediate reverse from over-rotation move off the boundary! aim_x={}",
            imu.aim_x()
        );
    }
}
