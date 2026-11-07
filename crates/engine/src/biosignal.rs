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
    // Muse 2 +X is nose/forward, +Y is left ear, +Z is top of head.
    // Project +X perpendicular to u0 to form the forward pitch reference axis f0.
    // If the sensor happens to have +X nearly parallel to u0, fall back to -Z.
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
    range_deg: f32,
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
            deadzone_deg: 1.2,
            range_deg: 22.0,
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
        if g_valid {
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
                if dev <= 6.5 {
                    self.still_sum[0] += raw_g[0];
                    self.still_sum[1] += raw_g[1];
                    self.still_sum[2] += raw_g[2];
                    self.still_count += 1;
                    self.still_time += dt;
                    if self.still_time >= 0.6 && self.still_count >= 4 {
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
                                self.yaw_zero_deg = if has_bridge_yaw { bridge_yaw } else { 0.0 };
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
                if norm3(w_check) < 3.0 {
                    let alpha_b = dt / (15.0 + dt);
                    self.gyro_bias[0] += alpha_b * (raw_g[0] - self.gyro_bias[0]);
                    self.gyro_bias[1] += alpha_b * (raw_g[1] - self.gyro_bias[1]);
                    self.gyro_bias[2] += alpha_b * (raw_g[2] - self.gyro_bias[2]);
                }
            }
        }

        let w = if g_valid && self.bias_calibrated {
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
        if self.rate_deg_s > 0.05 {
            let wx = w[0].to_radians();
            let wy = w[1].to_radians();
            let wz = w[2].to_radians();
            // du/dt = u x w in rotating body frame
            u = [
                u[0] + (u[1] * wz - u[2] * wy) * dt,
                u[1] + (u[2] * wx - u[0] * wz) * dt,
                u[2] + (u[0] * wy - u[1] * wx) * dt,
            ];
        }
        if let Some(m) = meas {
            let tilt_tau = if self.bias_calibrated { 0.45 } else { 0.08 };
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
            self.yaw_deg = bridge_yaw;
            self.yaw_zero_deg *= (-dt / 25.0).exp();
        } else {
            if self.rate_deg_s >= 1.5 {
                let yaw_rate = -dot3(w, u);
                self.yaw_deg = wrap_deg(self.yaw_deg + yaw_rate * dt);
            }
            let rel_y = wrap_deg(self.yaw_deg - self.yaw_zero_deg);
            let decayed = rel_y * (-dt / 25.0).exp();
            self.yaw_deg = wrap_deg(self.yaw_zero_deg + decayed);
        }
        self.raw_yaw = self.yaw_deg;

        if self.u0.is_none() {
            let (f0, r0) = build_head_basis(u);
            self.u0 = Some(u);
            self.f0 = f0;
            self.r0 = r0;
            self.yaw_zero_deg = self.yaw_deg;
        }

        let mut u0 = self.u0.unwrap();
        // Relative pitch & roll in the user's gravity-aligned tare frame (u0, f0, r0):
        // Positive rel_pitch = head pitched UP (nose up); Negative rel_pitch = head pitched DOWN.
        let cos_u = dot3(u, u0).clamp(-1.0, 1.0);
        let sin_pitch = dot3(u, self.f0);
        let sin_roll = dot3(u, self.r0);
        let mut dpitch = sin_pitch.atan2(cos_u).to_degrees();
        let droll = sin_roll.atan2(cos_u).to_degrees();
        let mut dyaw = wrap_deg(self.yaw_deg - self.yaw_zero_deg);

        // Posture / headband-shift protection:
        // 1) When stationary (rate < 2.2 deg/s), if |dpitch| > 16 deg or |dyaw| > 22 deg for > 0.9s
        //    (e.g. headset was just placed on forehead or adjusted), smoothly re-tare to current pose
        //    so the cursor can NEVER stay pinned at the top/bottom of the screen.
        // 2) During normal stationary rest (rate < 1.5 deg/s), slowly adapt u0 (tau = 10s) to absorb
        //    gradual chair slouching while keeping active head movements 1:1 responsive.
        if self.rate_deg_s < 2.2 {
            if dpitch.abs() > 16.0 || dyaw.abs() > 22.0 {
                self.edge_still_time += dt;
            } else {
                self.edge_still_time = (self.edge_still_time - dt * 0.5).max(0.0);
            }
            let tau = if self.edge_still_time > 0.9 { 0.35 } else { 10.0 };
            let beta = dt / (tau + dt);
            if let Some(new_u0) = normalize3([
                (1.0 - beta) * u0[0] + beta * u[0],
                (1.0 - beta) * u0[1] + beta * u[1],
                (1.0 - beta) * u0[2] + beta * u[2],
            ]) {
                let (f0, r0) = build_head_basis(new_u0);
                u0 = new_u0;
                self.u0 = Some(u0);
                self.f0 = f0;
                self.r0 = r0;
            }
            if self.edge_still_time > 0.9 {
                let dy = wrap_deg(self.yaw_deg - self.yaw_zero_deg);
                self.yaw_zero_deg = wrap_deg(self.yaw_zero_deg + dy * beta);
            }
            let cos_u2 = dot3(u, u0).clamp(-1.0, 1.0);
            dpitch = dot3(u, self.f0).atan2(cos_u2).to_degrees();
            dyaw = wrap_deg(self.yaw_deg - self.yaw_zero_deg);
        } else {
            self.edge_still_time = 0.0;
        }

        self.rel_pitch = dpitch;
        self.rel_roll = droll;
        self.rel_yaw = dyaw;

        let dz = self.deadzone_deg;
        let eff_yaw = if dyaw.abs() <= dz {
            0.0
        } else {
            dyaw.signum() * (dyaw.abs() - dz)
        };
        let eff_pitch = if dpitch.abs() <= dz {
            0.0
        } else {
            dpitch.signum() * (dpitch.abs() - dz)
        };

        let range = (self.range_deg / self.sensitivity.max(0.25)).max(4.0);
        let sx = if self.invert_yaw { -1.0 } else { 1.0 };
        // Screen Y is 0 at top, +1 at bottom.
        // Positive eff_pitch = looking UP -> negative tilt_y (top of screen).
        // Negative eff_pitch = looking DOWN -> positive tilt_y (bottom of screen).
        let sy = if self.invert_pitch { 1.0 } else { -1.0 };

        let target_x = clamp(sx * eff_yaw / range, -1.0, 1.0);
        let target_y = clamp(sy * eff_pitch / range, -1.0, 1.0);

        let k = 1.0 - (-14.0 * dt).exp();
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
            clamp(self.still_time / 0.6, 0.0, 1.0)
        }
    }
    pub fn is_tared(&self) -> bool {
        self.u0.is_some()
    }
}

const BLINK_DIM: usize = 6;
const ROLLING_EEG_SAMPLES: usize = 28; // ~0.44s at 64 Hz decimated EEG stream

#[derive(Clone, Copy)]
struct BlinkWindow {
    feat: [f32; BLINK_DIM],
    ptp_af7: f32,
    ptp_af8: f32,
    max_frontal: f32,
    corr: f32,
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
    xs.iter()
        .enumerate()
        .map(|(i, &y)| {
            if y.is_finite() {
                y - (mean_y + slope * (i as f32 - mean_x))
            } else {
                0.0
            }
        })
        .collect()
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

fn extract_blink_window(tp9: &[f32], af7: &[f32], af8: &[f32], tp10: &[f32]) -> BlinkWindow {
    let (ptp_af7, prom_7) = ptp_and_prominence(af7);
    let (ptp_af8, prom_8) = ptp_and_prominence(af8);
    let (ptp_tp9, _) = ptp_and_prominence(tp9);
    let (ptp_tp10, _) = ptp_and_prominence(tp10);

    let max_frontal = ptp_af7.max(ptp_af8);
    let max_temporal = ptp_tp9.max(ptp_tp10);
    let corr = pearson_corr(af7, af8);
    let prominence = prom_7.max(prom_8);

    let feat = [
        (ptp_af7 + 1.0).ln(),
        (ptp_af8 + 1.0).ln(),
        (max_frontal + 1.0).ln(),
        corr,
        prominence,
        ((max_frontal + 10.0) / (max_temporal + 10.0)).ln(),
    ];

    BlinkWindow {
        feat,
        ptp_af7,
        ptp_af8,
        max_frontal,
        corr,
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
    last_af7_ptp: f32,
    last_af8_ptp: f32,
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
            recent: Vec::with_capacity(20),
            positives: Vec::new(),
            negatives: Vec::new(),
            weights: [0.35, 0.35, 1.15, 0.55, 0.45, 0.40],
            bias: -0.2,
            feat_mean: [4.2, 4.2, 4.5, 0.1, 2.0, 0.2],
            feat_std: [1.0; BLINK_DIM],
            trained: false,
            accuracy: 0.0,
            learned_threshold_uv: 220.0,
            learned_af7_uv: 0.0,
            learned_af8_uv: 0.0,
            capture_timer: 0.0,
            cooldown: 0.0,
            rearmed: true,
            quiet_timer: 0.0,
            last_prob: 0.0,
            last_af7_ptp: 0.0,
            last_af8_ptp: 0.0,
            last_corr: 0.0,
            total_detected: 0,
        }
    }

    pub fn reset_model(&mut self) {
        self.positives.clear();
        self.negatives.clear();
        self.trained = false;
        self.accuracy = 0.0;
        self.learned_threshold_uv = 220.0;
        self.learned_af7_uv = 0.0;
        self.learned_af8_uv = 0.0;
        self.capture_timer = 0.0;
        self.last_prob = 0.0;
    }

    /// Starts or commits a blink calibration capture.
    /// If a prominent blink spike already occurred in the last ~1.0s (user blinked then clicked),
    /// it commits immediately; otherwise it stays armed for 1.5s so the user can blink right after clicking.
    pub fn record_blink_button(&mut self) -> bool {
        if let Some(best) = self.best_recent_candidate(130.0) {
            self.add_positive(best);
            self.capture_timer = 0.0;
            return true;
        }
        self.capture_timer = 1.5;
        false
    }

    pub fn record_baseline_button(&mut self) -> bool {
        if let Some(&last) = self.recent.last() {
            if last.max_frontal < 900.0 {
                self.add_negative(last);
                return true;
            }
        }
        false
    }

    fn best_recent_candidate(&self, min_uv: f32) -> Option<BlinkWindow> {
        let mut best: Option<BlinkWindow> = None;
        for w in &self.recent {
            if w.max_frontal >= min_uv && w.max_frontal <= 1800.0 {
                if best.map_or(true, |b| w.max_frontal > b.max_frontal) {
                    best = Some(*w);
                }
            }
        }
        best
    }

    fn add_positive(&mut self, w: BlinkWindow) {
        self.positives.push(w);
        if self.positives.len() > 80 {
            self.positives.remove(0);
        }
        if self.negatives.len() < 4 {
            for r in &self.recent {
                if r.max_frontal < w.max_frontal * 0.45 {
                    self.negatives.push(*r);
                }
            }
        }
        if self.negatives.len() < 3 {
            let synth = BlinkWindow {
                feat: [3.4, 3.4, 3.6, 0.0, 1.5, 0.0],
                ptp_af7: 30.0,
                ptp_af8: 30.0,
                max_frontal: 35.0,
                corr: 0.0,
            };
            self.negatives.push(synth);
        }
        self.train();
    }

    fn add_negative(&mut self, w: BlinkWindow) {
        self.negatives.push(w);
        if self.negatives.len() > 120 {
            self.negatives.remove(0);
        }
        if !self.positives.is_empty() {
            self.train();
        }
    }

    fn train(&mut self) {
        if self.positives.is_empty() {
            return;
        }
        let n_pos = self.positives.len();
        let n_neg = self.negatives.len();

        let mut af7_sum = 0.0f32;
        let mut af8_sum = 0.0f32;
        let mut max_sum = 0.0f32;
        for p in &self.positives {
            af7_sum += p.ptp_af7;
            af8_sum += p.ptp_af8;
            max_sum += p.max_frontal;
        }
        self.learned_af7_uv = af7_sum / n_pos as f32;
        self.learned_af8_uv = af8_sum / n_pos as f32;
        let pos_mean_max = max_sum / n_pos as f32;

        let neg_mean_max = if n_neg > 0 {
            self.negatives.iter().map(|w| w.max_frontal).sum::<f32>() / n_neg as f32
        } else {
            45.0
        };
        self.learned_threshold_uv = clamp(
            neg_mean_max + 0.48 * (pos_mean_max - neg_mean_max),
            95.0,
            750.0,
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
        let w_pos = total / (2.0 * n_pos as f32);
        let w_neg = total / (2.0 * n_neg as f32);
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
            if self.predict_window(sample) >= 0.5 {
                correct += 1;
            }
        }
        for sample in &self.negatives {
            if self.predict_window(sample) < 0.5 {
                correct += 1;
            }
        }
        self.accuracy = correct as f32 / (n_pos + n_neg) as f32;
    }

    fn predict_window(&self, w: &BlinkWindow) -> f32 {
        if w.max_frontal > 1800.0 || w.max_frontal < 65.0 {
            return 0.0;
        }
        if self.trained {
            let mut z = self.bias;
            for j in 0..BLINK_DIM {
                let x = (w.feat[j] - self.feat_mean[j]) / self.feat_std[j];
                z += self.weights[j] * x;
            }
            let lr_prob = 1.0 / (1.0 + (-z.clamp(-15.0, 15.0)).exp());
            let thresh_ratio = (w.max_frontal - self.learned_threshold_uv) / 60.0;
            let amp_prob = 1.0 / (1.0 + (-thresh_ratio.clamp(-15.0, 15.0)).exp());
            clamp(0.65 * lr_prob + 0.35 * amp_prob, 0.0, 1.0)
        } else {
            if w.corr < -0.25 {
                return 0.05;
            }
            let ratio = (w.max_frontal - self.learned_threshold_uv) / 55.0;
            let base_p = 1.0 / (1.0 + (-ratio.clamp(-12.0, 12.0)).exp());
            let corr_boost = if w.corr > 0.25 { 1.0 } else { 0.8 };
            clamp(base_p * corr_boost, 0.0, 1.0)
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

        if af7.is_empty() && af8.is_empty() {
            return false;
        }

        push_rolling(&mut self.buf_tp9, tp9, ROLLING_EEG_SAMPLES);
        push_rolling(&mut self.buf_af7, af7, ROLLING_EEG_SAMPLES);
        push_rolling(&mut self.buf_af8, af8, ROLLING_EEG_SAMPLES);
        push_rolling(&mut self.buf_tp10, tp10, ROLLING_EEG_SAMPLES);

        if self.buf_af7.len() < 4 && self.buf_af8.len() < 4 {
            return false;
        }

        let win = extract_blink_window(
            &self.buf_tp9,
            &self.buf_af7,
            &self.buf_af8,
            &self.buf_tp10,
        );
        self.last_af7_ptp = win.ptp_af7;
        self.last_af8_ptp = win.ptp_af8;
        self.last_corr = win.corr;

        self.recent.push(win);
        if self.recent.len() > 14 {
            self.recent.remove(0);
        }

        // If user clicked "Record Blink" and is blinking now within the capture window:
        if self.capture_timer > 0.0 {
            self.capture_timer = (self.capture_timer - dt).max(0.0);
            if self.capture_timer == 0.0 {
                if let Some(best) = self.best_recent_candidate(95.0) {
                    self.add_positive(best);
                }
            } else if win.max_frontal >= 160.0 && win.max_frontal <= 1800.0 && motion < 0.45 {
                self.add_positive(win);
                self.capture_timer = 0.0;
            }
        } else if motion < 0.2 && win.max_frontal > 12.0 && win.max_frontal < 85.0 {
            self.quiet_timer += dt;
            if self.quiet_timer >= 0.8 {
                self.quiet_timer = 0.0;
                self.add_negative(win);
            }
        }

        if win.max_frontal < self.learned_threshold_uv * 0.55 {
            self.rearmed = true;
        }

        let prob = if motion > 0.45 {
            0.0
        } else {
            self.predict_window(&win)
        };
        self.last_prob = prob;

        let trigger_thresh = if self.trained { 0.56 } else { 0.65 };
        if prob >= trigger_thresh && self.rearmed && self.cooldown <= 0.0 {
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
    pub fn last_af7_ptp(&self) -> f32 {
        self.last_af7_ptp
    }
    pub fn last_af8_ptp(&self) -> f32 {
        self.last_af8_ptp
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
    fn blink_classifier_learns_asymmetric_user_blinks() {
        let mut clf = RustBlinkClassifier::new();
        let quiet_7 = [-10.0, 5.0, 12.0, -8.0, 4.0, -3.0, 8.0, -5.0];
        let quiet_8 = [-8.0, 4.0, 10.0, -6.0, 3.0, -2.0, 6.0, -4.0];
        for _ in 0..10 {
            let fired = clf.ingest_frame(&quiet_7, &quiet_7, &quiet_8, &quiet_8, 0.02, 0.1);
            assert!(!fired);
        }
        let blink_7 = [-120.0, -80.0, 40.0, 300.0, 220.0, -40.0, -90.0, -50.0];
        let blink_8 = [-20.0, -12.0, 8.0, 45.0, 32.0, -5.0, -15.0, -8.0];
        clf.ingest_frame(&quiet_7, &blink_7, &blink_8, &quiet_8, 0.03, 0.1);

        let committed = clf.record_blink_button();
        assert!(committed);
        assert!(clf.is_trained());
        assert_eq!(clf.blink_samples(), 1);
        assert!(clf.learned_threshold_uv() < 300.0);

        for _ in 0..6 {
            clf.ingest_frame(&quiet_7, &quiet_7, &quiet_8, &quiet_8, 0.02, 0.1);
        }
        let detected = clf.ingest_frame(&quiet_7, &blink_7, &blink_8, &quiet_8, 0.03, 0.1);
        assert!(
            detected,
            "Trained Rust classifier should detect asymmetric blink (prob={})",
            clf.last_probability()
        );
    }
}
