//! Timing and rate meters for camera performance and debug telemetry (#33, #67, #108).
//! Pure logic port of src/camera/meter.js.

use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct RateMeter {
    pub window_ms: f64,
    pub times: VecDeque<f64>,
    pub count: usize,
}

impl Default for RateMeter {
    fn default() -> Self {
        Self::new(1000.0)
    }
}

impl RateMeter {
    pub fn new(window_ms: f64) -> Self {
        Self {
            window_ms,
            times: VecDeque::new(),
            count: 0,
        }
    }

    pub fn tick(&mut self, now: f64) {
        self.times.push_back(now);
        self.count += 1;
        self.prune(now);
    }

    fn prune(&mut self, now: f64) {
        while let Some(&first) = self.times.front() {
            if now - first > self.window_ms {
                self.times.pop_front();
            } else {
                break;
            }
        }
    }

    pub fn hz(&mut self, now: f64) -> f64 {
        self.prune(now);
        let n = self.times.len();
        if n < 2 {
            return 0.0;
        }
        let span = self.times.back().unwrap() - self.times.front().unwrap();
        if span > 0.0 {
            ((n - 1) as f64 * 1000.0) / span
        } else {
            0.0
        }
    }

    pub fn reset(&mut self) {
        self.times.clear();
        self.count = 0;
    }
}

#[derive(Debug, Clone)]
pub struct Ema {
    pub k: f64,
    pub value: Option<f64>,
}

impl Default for Ema {
    fn default() -> Self {
        Self::new(0.2)
    }
}

impl Ema {
    pub fn new(k: f64) -> Self {
        Self { k, value: None }
    }

    pub fn add(&mut self, x: f64) -> f64 {
        if x.is_finite() {
            self.value = Some(match self.value {
                None => x,
                Some(v) => v + (x - v) * self.k,
            });
        }
        self.value.unwrap_or(0.0)
    }

    pub fn reset(&mut self) {
        self.value = None;
    }
}

#[derive(Debug, Clone)]
pub struct FrameMeter {
    pub rate: RateMeter,
    pub long_ms: f64,
    pub last: Option<f64>,
    pub long_count: usize,
    pub worst: f64,
}

impl Default for FrameMeter {
    fn default() -> Self {
        Self::new(2000.0, 50.0)
    }
}

impl FrameMeter {
    pub fn new(window_ms: f64, long_ms: f64) -> Self {
        Self {
            rate: RateMeter::new(window_ms),
            long_ms,
            last: None,
            long_count: 0,
            worst: 0.0,
        }
    }

    pub fn frame(&mut self, now: f64) {
        if let Some(prev) = self.last {
            let dt = now - prev;
            if dt > self.long_ms && dt < 1000.0 {
                self.long_count += 1;
                self.worst = self.worst.max(dt);
            }
        }
        self.last = Some(now);
        self.rate.tick(now);
    }

    pub fn fps(&mut self, now: f64) -> f64 {
        self.rate.hz(now)
    }

    pub fn reset(&mut self) {
        self.rate.reset();
        self.last = None;
        self.long_count = 0;
        self.worst = 0.0;
    }
}
