//! Finger actions and smoothing for camera input (#33, #67, #108).
//! Pure logic port of src/camera/fingers.js.

use std::f64::consts::PI;

const TAU: f64 = 2.0 * PI;

#[inline]
fn alpha(cutoff: f64, dt: f64) -> f64 {
    let r = TAU * cutoff * dt;
    r / (r + 1.0)
}

#[inline]
pub fn clamp(v: f64) -> f64 {
    v.clamp(0.0, 1.0)
}

// ---------------------------------------------------------------------------
// OneEuroFilter
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct OneEuroFilter {
    pub min_cutoff: f64,
    pub beta: f64,
    pub d_cutoff: f64,
    pub max_gap_ms: f64,
    pub x: Option<f64>,
    pub dx: f64,
    pub t: Option<f64>,
}

impl Default for OneEuroFilter {
    fn default() -> Self {
        Self::new(1.2, 8.0, 1.0, 500.0)
    }
}

impl OneEuroFilter {
    pub fn new(min_cutoff: f64, beta: f64, d_cutoff: f64, max_gap_ms: f64) -> Self {
        Self {
            min_cutoff,
            beta,
            d_cutoff,
            max_gap_ms,
            x: None,
            dx: 0.0,
            t: None,
        }
    }

    pub fn reset(&mut self) {
        self.x = None;
        self.dx = 0.0;
        self.t = None;
    }

    pub fn filter(&mut self, value: f64, t_ms: f64) -> f64 {
        if self.x.is_none() || (t_ms - self.t.unwrap_or(0.0) > self.max_gap_ms) {
            self.x = Some(value);
            self.dx = 0.0;
            self.t = Some(t_ms);
            return value;
        }

        let prev_t = self.t.unwrap();
        let dt = (t_ms - prev_t) / 1000.0;
        if !(dt > 0.0) {
            return self.x.unwrap();
        }

        let prev_x = self.x.unwrap();
        self.t = Some(t_ms);
        self.dx += alpha(self.d_cutoff, dt) * ((value - prev_x) / dt - self.dx);
        let cutoff = self.min_cutoff + self.beta * self.dx.abs();
        let new_x = prev_x + alpha(cutoff, dt) * (value - prev_x);
        self.x = Some(new_x);
        new_x
    }
}

// ---------------------------------------------------------------------------
// PointFilter (2D)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct PointFilter {
    pub fx: OneEuroFilter,
    pub fy: OneEuroFilter,
}

impl PointFilter {
    pub fn new(min_cutoff: f64, beta: f64, d_cutoff: f64, max_gap_ms: f64) -> Self {
        Self {
            fx: OneEuroFilter::new(min_cutoff, beta, d_cutoff, max_gap_ms),
            fy: OneEuroFilter::new(min_cutoff, beta, d_cutoff, max_gap_ms),
        }
    }

    pub fn reset(&mut self) {
        self.fx.reset();
        self.fy.reset();
    }

    pub fn filter(&mut self, p: (f64, f64), t_ms: f64) -> (f64, f64) {
        (self.fx.filter(p.0, t_ms), self.fy.filter(p.1, t_ms))
    }
}

// ---------------------------------------------------------------------------
// Soft Dead Zone
// ---------------------------------------------------------------------------

pub fn soft_dead_zone(out: Option<(f64, f64)>, target: (f64, f64), radius: f64) -> (f64, f64) {
    match out {
        None => target,
        Some(o) => {
            let dx = target.0 - o.0;
            let dy = target.1 - o.1;
            let d = dx.hypot(dy);
            if d <= radius {
                o
            } else {
                let k = (d - radius) / d;
                (o.0 + dx * k, o.1 + dy * k)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// HandRange
// ---------------------------------------------------------------------------

pub const FIXED_RANGE_X: (f64, f64) = (0.15, 0.85);
pub const FIXED_RANGE_Y: (f64, f64) = (0.12, 0.72);

#[derive(Debug, Clone)]
pub struct HandRange {
    pub learn_ms: f64,
    pub tau_ms: f64,
    pub min_span: (f64, f64),
    pub margin: f64,
    pub fixed_x: (f64, f64),
    pub fixed_y: (f64, f64),
    pub max_step_ms: f64,
    pub lo: Option<(f64, f64)>,
    pub hi: Option<(f64, f64)>,
    pub seen_ms: f64,
    pub last_t: Option<f64>,
}

impl Default for HandRange {
    fn default() -> Self {
        Self {
            learn_ms: 3000.0,
            tau_ms: 30000.0,
            min_span: (0.2, 0.16),
            margin: 0.08,
            fixed_x: FIXED_RANGE_X,
            fixed_y: FIXED_RANGE_Y,
            max_step_ms: 200.0,
            lo: None,
            hi: None,
            seen_ms: 0.0,
            last_t: None,
        }
    }
}

impl HandRange {
    pub fn reset(&mut self) {
        self.lo = None;
        self.hi = None;
        self.seen_ms = 0.0;
        self.last_t = None;
    }

    pub fn progress(&self) -> f64 {
        if self.learn_ms > 0.0 {
            clamp(self.seen_ms / self.learn_ms)
        } else {
            1.0
        }
    }

    pub fn calibrated(&self) -> bool {
        self.seen_ms >= self.learn_ms
    }

    pub fn update(&mut self, p: (f64, f64), t_ms: f64) {
        let x = clamp(p.0);
        let y = clamp(p.1);
        let dt = match self.last_t {
            None => 0.0,
            Some(lt) => (t_ms - lt).clamp(0.0, self.max_step_ms),
        };
        self.last_t = Some(t_ms);

        if self.lo.is_none() {
            self.lo = Some((x, y));
            self.hi = Some((x, y));
            return;
        }

        let learned = self.calibrated();
        self.seen_ms += dt;
        let k = if learned && dt > 0.0 {
            1.0 - (-dt / self.tau_ms).exp()
        } else {
            0.0
        };

        let (mut lox, mut loy) = self.lo.unwrap();
        let (mut hix, mut hiy) = self.hi.unwrap();

        lox = lox.min(x);
        loy = loy.min(y);
        hix = hix.max(x);
        hiy = hiy.max(y);

        if k > 0.0 {
            lox += (x - lox) * k;
            loy += (y - loy) * k;
            hix += (x - hix) * k;
            hiy += (y - hiy) * k;
        }

        self.lo = Some((lox, loy));
        self.hi = Some((hix, hiy));
    }

    pub fn bounds(&self, is_x: bool) -> (f64, f64) {
        let (fixed_0, fixed_1) = if is_x { self.fixed_x } else { self.fixed_y };
        if self.lo.is_none() {
            return (fixed_0, fixed_1);
        }

        let (mut a, mut b) = if is_x {
            (self.lo.unwrap().0, self.hi.unwrap().0)
        } else {
            (self.lo.unwrap().1, self.hi.unwrap().1)
        };

        let min = if is_x { self.min_span.0 } else { self.min_span.1 };
        if b - a < min {
            let c = (a + b) / 2.0;
            a = c - min / 2.0;
            b = c + min / 2.0;
        }

        let m = (b - a) * self.margin;
        a += m;
        b -= m;

        let w = self.progress();
        (fixed_0 + (a - fixed_0) * w, fixed_1 + (b - fixed_1) * w)
    }

    pub fn map(&self, p: (f64, f64), clamped: bool) -> (f64, f64) {
        let (x0, x1) = self.bounds(true);
        let (y0, y1) = self.bounds(false);
        let x = (p.0 - x0) / (x1 - x0);
        let y = (p.1 - y0) / (y1 - y0);
        if clamped {
            (clamp(x), clamp(y))
        } else {
            (x, y)
        }
    }
}

// ---------------------------------------------------------------------------
// Pinch Detection
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub struct Landmark {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[inline]
fn dist3(a: Landmark, b: Landmark, aspect: f64) -> f64 {
    let dx = (a.x - b.x) * aspect;
    let dy = a.y - b.y;
    let dz = (a.z - b.z) * aspect;
    (dx * dx + dy * dy + dz * dz).sqrt()
}

#[derive(Debug, Clone, Copy)]
pub struct PinchMetrics {
    pub ratio: f64,
    pub reach: f64,
    pub point: (f64, f64),
}

pub fn pinch_metrics(hand: &[Landmark], aspect: f64) -> Option<PinchMetrics> {
    if hand.len() < 21 {
        return None;
    }
    let d_0_9 = dist3(hand[0], hand[9], aspect);
    let d_5_17 = 1.4 * dist3(hand[5], hand[17], aspect);
    let size = d_0_9.max(d_5_17).max(1e-6);

    let d_4_8 = dist3(hand[4], hand[8], aspect);
    let d_0_8 = dist3(hand[0], hand[8], aspect);
    let d_0_5 = dist3(hand[0], hand[5], aspect).max(1e-6);

    Some(PinchMetrics {
        ratio: d_4_8 / size,
        reach: d_0_8 / d_0_5,
        point: ((hand[4].x + hand[8].x) / 2.0, (hand[4].y + hand[8].y) / 2.0),
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PinchEvent {
    pub active: bool,
    pub started: bool,
    pub released: bool,
    pub cancelled: bool,
    pub held_ms: f64,
}

#[derive(Debug, Clone)]
pub struct PinchDetector {
    pub enter: f64,
    pub exit: f64,
    pub enter_ms: f64,
    pub exit_ms: f64,
    pub loss_ms: f64,
    pub min_reach: f64,
    pub active: bool,
    pub since: f64,
    pub closed_at: Option<f64>,
    pub open_at: Option<f64>,
    pub last_seen: f64,
}

impl Default for PinchDetector {
    fn default() -> Self {
        Self {
            enter: 0.3,
            exit: 0.45,
            enter_ms: 50.0,
            exit_ms: 60.0,
            loss_ms: 300.0,
            min_reach: 0.9,
            active: false,
            since: 0.0,
            closed_at: None,
            open_at: None,
            last_seen: f64::NEG_INFINITY,
        }
    }
}

impl PinchDetector {
    pub fn reset(&mut self) {
        self.active = false;
        self.since = 0.0;
        self.closed_at = None;
        self.open_at = None;
        self.last_seen = f64::NEG_INFINITY;
    }

    pub fn update(&mut self, ratio: Option<f64>, now: f64, can_start: bool) -> PinchEvent {
        let mut ev = PinchEvent {
            active: false,
            started: false,
            released: false,
            cancelled: false,
            held_ms: 0.0,
        };

        match ratio {
            None => {
                self.closed_at = None;
                self.open_at = None;
                if self.active && now - self.last_seen > self.loss_ms {
                    self.active = false;
                    ev.cancelled = true;
                    ev.held_ms = now - self.since;
                }
            }
            Some(r) => {
                self.last_seen = now;
                if !self.active {
                    if r < self.enter && can_start {
                        if self.closed_at.is_none() {
                            self.closed_at = Some(now);
                        }
                        if now - self.closed_at.unwrap() >= self.enter_ms {
                            self.active = true;
                            self.since = now;
                            self.open_at = None;
                            ev.started = true;
                        }
                    } else {
                        self.closed_at = None;
                    }
                } else if r > self.exit {
                    if self.open_at.is_none() {
                        self.open_at = Some(now);
                    }
                    if now - self.open_at.unwrap() >= self.exit_ms {
                        self.active = false;
                        self.closed_at = None;
                        ev.released = true;
                        ev.held_ms = now - self.since;
                    }
                } else {
                    self.open_at = None;
                }
            }
        }

        ev.active = self.active;
        if ev.active {
            ev.held_ms = now - self.since;
        }
        ev
    }
}

// ---------------------------------------------------------------------------
// FingerController
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FingerMode {
    None,
    Point,
    Pinch,
    Release,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FingerUpdate {
    pub mode: FingerMode,
    pub aim: Option<(f64, f64)>,
    pub cast: bool,
    pub cancelled: bool,
    pub suppress: bool,
    pub progress: f64,
}

#[derive(Debug, Clone)]
pub struct FingerController {
    pub aim_score: f64,
    pub point_grace_ms: f64,
    pub precise_gain: f64,
    pub dead_zone: f64,
    pub min_pinch_ms: f64,
    pub refractory_ms: f64,
    pub fist_score: f64,

    pub filter: PointFilter,
    pub pinch_filter: PointFilter,
    pub range: HandRange,
    pub pinch: PinchDetector,

    pub aim: Option<(f64, f64)>,
    pub pinch_anchor: Option<(f64, f64)>,
    pub aim_anchor: Option<(f64, f64)>,
    pub last_seen_hand: f64,
    pub last_point_seen: f64,
    pub released_at: f64,
}

impl Default for FingerController {
    fn default() -> Self {
        Self {
            aim_score: 0.45,
            point_grace_ms: 200.0,
            precise_gain: 0.35,
            dead_zone: 0.004,
            min_pinch_ms: 150.0,
            refractory_ms: 300.0,
            fist_score: 0.5,
            filter: PointFilter::new(1.2, 8.0, 1.0, 500.0),
            pinch_filter: PointFilter::new(0.6, 4.0, 1.0, 500.0),
            range: HandRange::default(),
            pinch: PinchDetector::default(),
            aim: None,
            pinch_anchor: None,
            aim_anchor: None,
            last_seen_hand: f64::NEG_INFINITY,
            last_point_seen: f64::NEG_INFINITY,
            released_at: f64::NEG_INFINITY,
        }
    }
}

impl FingerController {
    pub fn reset(&mut self) {
        self.filter.reset();
        self.pinch_filter.reset();
        self.range.reset();
        self.pinch.reset();
        self.aim = None;
        self.pinch_anchor = None;
        self.aim_anchor = None;
        self.last_seen_hand = f64::NEG_INFINITY;
        self.last_point_seen = f64::NEG_INFINITY;
        self.released_at = f64::NEG_INFINITY;
    }

    pub fn update(
        &mut self,
        hand: Option<&[Landmark]>,
        gesture: &str,
        score: f64,
        now: f64,
        aspect: f64,
    ) -> FingerUpdate {
        let mut mode = FingerMode::None;
        let mut cast = false;
        let mut cancelled = false;

        let metrics = hand.and_then(|h| pinch_metrics(h, aspect));

        if let Some(h) = hand {
            if h.len() >= 21 {
                self.last_seen_hand = now;
                let mirrored_x = 1.0 - h[8].x;
                let raw_point = (mirrored_x, h[8].y);
                self.range.update(raw_point, now);
            }
        }

        let is_fist = gesture == "Closed_Fist" && score >= self.fist_score;
        let can_start_pinch = metrics.map_or(false, |m| m.reach >= self.pinch.min_reach) && !is_fist;
        let ratio = metrics.map(|m| m.ratio);
        let pinch_ev = self.pinch.update(ratio, now, can_start_pinch);

        if pinch_ev.started {
            if let Some(m) = metrics {
                let mirrored_pt = (1.0 - m.point.0, m.point.1);
                let smoothed = self.pinch_filter.filter(mirrored_pt, now);
                self.pinch_anchor = Some(smoothed);
                self.aim_anchor = self.aim;
            }
        }

        if pinch_ev.active {
            mode = FingerMode::Pinch;
            if let Some(m) = metrics {
                let mirrored_pt = (1.0 - m.point.0, m.point.1);
                let smoothed = self.pinch_filter.filter(mirrored_pt, now);
                let anchor = self.pinch_anchor.unwrap_or(smoothed);
                let delta = (
                    (smoothed.0 - anchor.0) * self.precise_gain,
                    (smoothed.1 - anchor.1) * self.precise_gain,
                );
                let base = self.aim_anchor.unwrap_or((0.5, 0.5));
                let new_aim = (clamp(base.0 + delta.0), clamp(base.1 + delta.1));
                self.aim = Some(new_aim);
            }
        } else if pinch_ev.released {
            self.released_at = now;
            if pinch_ev.held_ms >= self.min_pinch_ms {
                mode = FingerMode::Release;
                cast = true;
            } else {
                cancelled = true;
            }
            self.pinch_anchor = None;
            self.aim_anchor = None;
        } else if pinch_ev.cancelled {
            cancelled = true;
            self.pinch_anchor = None;
            self.aim_anchor = None;
        } else {
            let is_pointing = gesture == "Pointing_Up" && score >= self.aim_score;
            if is_pointing {
                self.last_point_seen = now;
            }
            let active_point = is_pointing || (now - self.last_point_seen <= self.point_grace_ms);

            if active_point && hand.is_some() && hand.unwrap().len() >= 21 {
                mode = FingerMode::Point;
                let h = hand.unwrap();
                let mirrored_x = 1.0 - h[8].x;
                let smoothed = self.filter.filter((mirrored_x, h[8].y), now);
                let dead_zoned = soft_dead_zone(self.aim, smoothed, self.dead_zone);
                let mapped = self.range.map(dead_zoned, true);
                self.aim = Some(mapped);
            }
        }

        let suppress = pinch_ev.active
            || (now - self.released_at < self.refractory_ms)
            || self.pinch.closed_at.is_some();

        let progress = if pinch_ev.active {
            (pinch_ev.held_ms / self.min_pinch_ms).clamp(0.0, 1.0)
        } else {
            0.0
        };

        FingerUpdate {
            mode,
            aim: self.aim,
            cast,
            cancelled,
            suppress,
            progress,
        }
    }
}
