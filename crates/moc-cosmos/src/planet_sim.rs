//! Pure planet simulation engine.
//!
//! Evaluates multi-world progression driven by biometric channels (calm, focus, pulse, blink).
//! Reference implementation matches `src/cosmos/planet-sim.js`.

use crate::math::{clamp, Mulberry32};

pub const ERA_COUNT: usize = 4;
pub const ERA_SECONDS: f32 = 15.0;
pub const POP_CAP: [f32; ERA_COUNT] = [60.0, 400.0, 3000.0, 12000.0];
pub const MIN_POP: f32 = 3.0;
pub const OFF_CHANNEL: f32 = 0.25;

pub const GROWTH_BASE: f32 = 0.03;
pub const GROWTH_CALM: f32 = 0.2;
pub const GROWTH_STRESS: f32 = 0.1;

pub const RESEARCH_BASE: f32 = 0.45;
pub const RESEARCH_FOCUS: f32 = 1.5;
pub const RESEARCH_CALM: f32 = 0.2;
pub const RESEARCH_STRESS: f32 = 0.4;
pub const RESEARCH_MIN: f32 = 0.1;
pub const RESEARCH_MAX: f32 = 2.4;

pub const MARKET_START: f32 = 100.0;
pub const MARKET_REVERT: f32 = 0.15;
pub const MARKET_SIGMA_BASE: f32 = 0.02;
pub const MARKET_SIGMA_PULSE: f32 = 0.22;
pub const MARKET_PEAK_DECAY: f32 = 0.05;

pub const CRASH_RATIO: f32 = 0.78;
pub const CRASH_RESET: f32 = 0.92;
pub const BOOM_LEVEL: f32 = 128.0;
pub const BOOM_RESET: f32 = 115.0;

pub const CRASH_POP: f32 = 0.92;
pub const CRASH_PROGRESS: f32 = 0.2;
pub const BOOM_PROGRESS: f32 = 0.15;

pub const EVENT_MIN: f32 = 4.0;
pub const EVENT_MAX: f32 = 7.0;

pub const CALM_HIGH: f32 = 0.55;
pub const CALM_LOW: f32 = 0.12;
pub const CALM_LOW_CHANCE: f32 = 0.5;
pub const HARVEST_GAIN: f32 = 1.15;
pub const FAMINE_LOSS: f32 = 0.15;

pub const FOCUS_HIGH: f32 = 0.55;
pub const FOCUS_LOW: f32 = 0.10;
pub const FOCUS_LOW_CHANCE: f32 = 0.35;
pub const BREAKTHROUGH: f32 = 0.2;
pub const DARK_AGE: f32 = 0.15;
pub const FLAVOUR_CHANCE: f32 = 0.25;

pub const STORM_CHANCE_OFF: f32 = 0.12;
pub const STORM_POP_OWN: f32 = 0.97;
pub const STORM_POP_OFF: f32 = 0.99;
pub const STORM_PROGRESS_OWN: f32 = 0.05;
pub const STORM_PROGRESS_OFF: f32 = 0.01;

pub const LAUNCH_EVERY: f32 = 7.0;
pub const SETTLE_POP: f32 = 12.0;
pub const REINFORCE_POP: f32 = 8.0;
pub const REINFORCE_PROGRESS: f32 = 0.1;

pub const STRESS_BPM: f32 = 20.0;
pub const STRESS_MAX: f32 = 1.5;
pub const MIND_SECONDS: f32 = 10.0;

pub const ERA_TITLES: [&str; ERA_COUNT] = ["Stone Age", "Farming", "Industry", "Space Age"];

pub const FLAVOUR: [&[&str]; ERA_COUNT] = [
    &["invented the spear", "worships a rock", "met a mammoth"],
    &["domesticated goats", "invented cheese", "elected a chicken"],
    &[
        "opened a goat exchange",
        "invented the strike",
        "built a mill",
    ],
    &[
        "launched a satellite",
        "found water on a moon",
        "built a goat in orbit",
    ],
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Channel {
    Calm,
    Focus,
    Pulse,
    Blink,
    Mixed,
}

impl Channel {
    pub fn name(self) -> &'static str {
        match self {
            Channel::Calm => "calm",
            Channel::Focus => "focus",
            Channel::Pulse => "pulse",
            Channel::Blink => "blink",
            Channel::Mixed => "mixed",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct EffortSplit {
    pub growth: f32,
    pub research: f32,
    pub stability: f32,
}

impl EffortSplit {
    pub fn for_channel(ch: Channel) -> Self {
        match ch {
            Channel::Calm => Self {
                growth: 0.55,
                research: 0.25,
                stability: 0.20,
            },
            Channel::Focus => Self {
                growth: 0.20,
                research: 0.60,
                stability: 0.20,
            },
            Channel::Pulse => Self {
                growth: 0.40,
                research: 0.50,
                stability: 0.10,
            },
            Channel::Blink => Self {
                growth: 0.30,
                research: 0.40,
                stability: 0.30,
            },
            Channel::Mixed => Self {
                growth: 0.40,
                research: 0.40,
                stability: 0.20,
            },
        }
    }

    pub fn normalised(&self) -> Self {
        let g = self.growth.max(0.0);
        let r = self.research.max(0.0);
        let s = self.stability.max(0.0);
        let total = (g + r + s).max(1e-4);
        Self {
            growth: g / total,
            research: r / total,
            stability: s / total,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct BrainEffects {
    pub calm: f32,
    pub focus: f32,
    pub hr_delta: f32,
    pub stress: f32,
    pub blink_rate: f32,
}

impl BrainEffects {
    pub fn from_signals(calm: f32, focus: f32, hr_delta: f32, blinks_last_10s: u32) -> Self {
        let stress = clamp(hr_delta / STRESS_BPM, 0.0, STRESS_MAX);
        let blink_rate = blinks_last_10s as f32 / 10.0;
        Self {
            calm,
            focus,
            hr_delta,
            stress,
            blink_rate,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PlanetState {
    pub channel: Channel,
    pub pop: f32,
    pub era_index: usize,
    pub progress: f32,
    pub market: f32,
    pub market_peak: f32,
    pub crashed: bool,
    pub boomed: bool,
    pub event_t: f32,
    pub launch_t: f32,
    pub storms: u32,
    pub alloc: Option<EffortSplit>,
}

impl PlanetState {
    pub fn new(channel: Channel, pop: f32, era_index: usize, rng: &mut Mulberry32) -> Self {
        Self {
            channel,
            pop,
            era_index,
            progress: 0.0,
            market: MARKET_START,
            market_peak: MARKET_START,
            crashed: false,
            boomed: false,
            event_t: rng.range_f32(EVENT_MIN, EVENT_MAX),
            launch_t: 0.0,
            storms: 0,
            alloc: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum EventKind {
    Era,
    Good,
    Bad,
    Flavour,
    LaunchReady,
    Storm,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SimEvent {
    pub kind: EventKind,
    pub text: String,
    pub cause: Option<String>,
}

pub struct StepResult {
    pub events: Vec<SimEvent>,
    pub growth: f32,
    pub research: f32,
}

/// Advance one inhabited world by dt seconds.
pub fn step_planet(
    p: &mut PlanetState,
    dt: f32,
    fx: &BrainEffects,
    rng: &mut Mulberry32,
    attended: bool,
) -> StepResult {
    let mut events = Vec::new();
    if p.pop <= 0.0 {
        return StepResult {
            events,
            growth: 0.0,
            research: 0.0,
        };
    }

    let ch = p.channel;
    let w = |target: Channel| -> f32 {
        if attended || ch == target || ch == Channel::Mixed {
            1.0
        } else {
            OFF_CHANNEL
        }
    };

    let alloc = p
        .alloc
        .unwrap_or_else(|| EffortSplit::for_channel(ch))
        .normalised();
    let cap = POP_CAP[p.era_index];

    // Population growth: logistic growth driven by calm, dragged by stress
    let growth = (GROWTH_BASE + GROWTH_CALM * w(Channel::Calm) * fx.calm
        - GROWTH_STRESS * w(Channel::Pulse) * fx.stress)
        * (0.5 + alloc.growth);
    p.pop = clamp(
        p.pop + p.pop * growth * (1.0 - p.pop / cap) * dt,
        MIN_POP,
        cap,
    );

    // Research rate: focus speeds up, stress slows down
    let raw_research = RESEARCH_BASE + RESEARCH_FOCUS * w(Channel::Focus) * fx.focus
        + RESEARCH_CALM * fx.calm
        - RESEARCH_STRESS * w(Channel::Pulse) * fx.stress;
    let research = clamp(raw_research, RESEARCH_MIN, RESEARCH_MAX) * (0.5 + alloc.research);
    p.progress += (dt / ERA_SECONDS) * research;

    // Goat market log-price stochastic process: volatility is the player's pulse
    let sigma = MARKET_SIGMA_BASE + MARKET_SIGMA_PULSE * w(Channel::Pulse) * fx.stress;
    let d_log_m = MARKET_REVERT * (MARKET_START / p.market).ln() * dt
        + sigma * dt.sqrt() * rng.gauss();
    p.market *= d_log_m.exp();
    p.market_peak = p
        .market_peak
        .max(p.market_peak - (p.market_peak - p.market) * MARKET_PEAK_DECAY * dt)
        .max(p.market);

    // Era completion
    if p.progress >= 1.0 && p.era_index < ERA_COUNT - 1 {
        p.era_index += 1;
        p.progress = 0.0;
        events.push(SimEvent {
            kind: EventKind::Era,
            text: ERA_TITLES[p.era_index].to_string(),
            cause: None,
        });
    }
    p.progress = p.progress.min(1.0);

    let steady = 1.0 - 0.5 * alloc.stability; // stability softens setbacks

    // Scheduled events
    p.event_t -= dt;
    if p.event_t <= 0.0 {
        p.event_t = rng.range_f32(EVENT_MIN, EVENT_MAX);

        // Calm events (bumper harvest vs famine)
        if ch == Channel::Calm || ch == Channel::Mixed {
            if fx.calm > CALM_HIGH {
                p.pop = cap.min(p.pop * HARVEST_GAIN);
                events.push(SimEvent {
                    kind: EventKind::Good,
                    text: "bumper harvest".to_string(),
                    cause: Some("a calm god".to_string()),
                });
            } else if fx.calm < CALM_LOW && rng.chance(CALM_LOW_CHANCE) {
                p.pop = MIN_POP.max(p.pop * (1.0 - FAMINE_LOSS * steady));
                events.push(SimEvent {
                    kind: EventKind::Bad,
                    text: if p.era_index == 0 {
                        "famine".to_string()
                    } else {
                        "poor harvest".to_string()
                    },
                    cause: Some("a restless god".to_string()),
                });
            }
        }

        // Focus events (breakthrough vs dark age)
        if ch == Channel::Focus || ch == Channel::Mixed {
            if fx.focus > FOCUS_HIGH {
                p.progress = 1.0f32.min(p.progress + BREAKTHROUGH);
                events.push(SimEvent {
                    kind: EventKind::Good,
                    text: "breakthrough".to_string(),
                    cause: Some("your focus".to_string()),
                });
            } else if fx.focus < FOCUS_LOW && rng.chance(FOCUS_LOW_CHANCE) {
                p.progress = 0.0f32.max(p.progress - DARK_AGE * steady);
                events.push(SimEvent {
                    kind: EventKind::Bad,
                    text: "dark age".to_string(),
                    cause: Some("a wandering mind".to_string()),
                });
            }
        }

        // Flavour event
        if rng.chance(FLAVOUR_CHANCE) {
            let item = rng.pick(FLAVOUR[p.era_index]);
            events.push(SimEvent {
                kind: EventKind::Flavour,
                text: item.to_string(),
                cause: None,
            });
        }
    }

    // Pulse events (goat market boom & crash)
    if ch == Channel::Pulse || ch == Channel::Mixed {
        if !p.crashed && p.market < p.market_peak * CRASH_RATIO {
            p.crashed = true;
            p.pop = MIN_POP.max(p.pop * (1.0 - (1.0 - CRASH_POP) * steady));
            p.progress = 0.0f32.max(p.progress - CRASH_PROGRESS * steady);
            let sign = if fx.hr_delta >= 0.0 { "+" } else { "" };
            events.push(SimEvent {
                kind: EventKind::Bad,
                text: "goat crash".to_string(),
                cause: Some(format!("your pulse at {}{:.0} bpm", sign, fx.hr_delta)),
            });
        } else if !p.boomed && p.market > BOOM_LEVEL {
            p.boomed = true;
            p.progress = 1.0f32.min(p.progress + BOOM_PROGRESS);
            events.push(SimEvent {
                kind: EventKind::Good,
                text: "goat boom".to_string(),
                cause: Some("a lively pulse".to_string()),
            });
        }

        if p.market > p.market_peak * CRASH_RESET {
            p.crashed = false;
        }
        if p.market < BOOM_RESET {
            p.boomed = false;
        }
    }

    // Space Age launch readiness
    if p.era_index == ERA_COUNT - 1 {
        p.launch_t += dt * (0.6 + fx.focus);
        if p.launch_t >= LAUNCH_EVERY {
            p.launch_t = 0.0;
            events.push(SimEvent {
                kind: EventKind::LaunchReady,
                text: "space launch ready".to_string(),
                cause: None,
            });
        }
    }

    StepResult {
        events,
        growth,
        research,
    }
}

/// A player blink strikes the world.
pub fn blink_planet(
    p: &mut PlanetState,
    rng: &mut Mulberry32,
    attended: bool,
) -> Option<SimEvent> {
    if p.pop <= 0.0 {
        return None;
    }
    let own = attended || p.channel == Channel::Blink;
    if !own && !rng.chance(STORM_CHANCE_OFF) {
        return None;
    }

    p.storms += 1;
    p.pop = MIN_POP.max(p.pop * (if own { STORM_POP_OWN } else { STORM_POP_OFF }));
    p.progress =
        1.0f32.min(p.progress + (if own { STORM_PROGRESS_OWN } else { STORM_PROGRESS_OFF }));

    let text = if attended {
        "lightning strike!".to_string()
    } else if own {
        rng.pick(&["storm!", "lightning!", "thunderstorm"]).to_string()
    } else {
        "stray bolt".to_string()
    };

    Some(SimEvent {
        kind: if own { EventKind::Storm } else { EventKind::Flavour },
        text,
        cause: None,
    })
}

/// Settle a barren world or reinforce an inhabited one with a rocket ship.
pub fn arrive_ship(to: &mut PlanetState) -> &'static str {
    if to.pop <= 0.0 {
        to.pop = SETTLE_POP;
        to.era_index = 0;
        to.progress = 0.0;
        "settled"
    } else {
        to.pop += REINFORCE_POP;
        to.progress = 1.0f32.min(to.progress + REINFORCE_PROGRESS);
        "reinforced"
    }
}

/// Mind snapshot accumulator for the first 10 seconds of play.
#[derive(Debug, Clone, Default)]
pub struct MindSnapshot {
    pub t: f32,
    pub calm: f32,
    pub focus: f32,
    pub hr: f32,
    pub blinks_start: Option<u32>,
    pub blinks: u32,
    pub hr_delta: f32,
    pub done: bool,
    pub ranked: Option<Vec<(Channel, f32)>>,
}

impl MindSnapshot {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(
        &mut self,
        dt: f32,
        calm: f32,
        focus: f32,
        hr_delta: f32,
        blinks_total: u32,
    ) -> Option<&[(Channel, f32)]> {
        if self.done || dt <= 0.0 {
            return self.ranked.as_deref();
        }

        if self.blinks_start.is_none() {
            self.blinks_start = Some(blinks_total);
        }

        self.t += dt;
        self.calm += calm * dt;
        self.focus += focus * dt;
        self.hr += hr_delta * dt;

        if self.t < MIND_SECONDS {
            return None;
        }

        self.done = true;
        let avg_calm = self.calm / self.t;
        let avg_focus = self.focus / self.t;
        let avg_hr_delta = self.hr / self.t;
        let blinks = blinks_total.saturating_sub(self.blinks_start.unwrap_or(0));

        let score_calm = avg_calm / 0.35;
        let score_focus = avg_focus / 0.30;
        let score_pulse = avg_hr_delta.max(0.0) / 8.0;
        let score_blink = blinks as f32 / 3.0;

        let mut ranked = vec![
            (Channel::Calm, score_calm),
            (Channel::Focus, score_focus),
            (Channel::Pulse, score_pulse),
            (Channel::Blink, score_blink),
        ];

        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        self.ranked = Some(ranked);
        self.ranked.as_deref()
    }
}
