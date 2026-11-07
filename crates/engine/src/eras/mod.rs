pub mod game;
pub mod industry;
pub mod space;
pub mod stone;

#[derive(Clone, Debug, PartialEq)]
pub struct Objective {
    pub label: String,
    pub value: String,
    pub done: bool,
    pub bad: bool,
    pub plain: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SoundKind {
    Thunder,
    Ignite,
    Blip,
    Good,
    Bad,
    Pop,
    Coin,
    Bell,
    Crash,
    Tick,
    Rumble,
    Fanfare,
}

#[derive(Clone, Debug, PartialEq)]
pub enum EraEvent {
    AdvisorSay {
        key: String,
        text: String,
        mood: String,
        priority: u8,
        cooldown: f32,
    },
    PlaySound(SoundKind),
    StatAddLightning,
    StatAddSmitten,
    StatAddBirth,
    StatAddDeath,
    StatAddRecession,
}

/// What the eras read from the player each tick. `stress` is the JS brain's
/// `brain.stress = max(1 - calm, face anger)`, 0..1: the one stress signal the game uses.
#[derive(Clone, Copy, Debug, Default)]
pub struct BrainInput {
    pub calm: f32,
    pub focus: f32,
    pub stress: f32,
    pub blinks: u32,
}

/// Stress at or below this counts as calm enough: 1 - the 0.55 calm bar the JS eras use.
pub const STRESS_LIMIT: f32 = 0.45;

pub trait Era {
    fn id(&self) -> &str;
    fn label(&self) -> &str;
    fn powers(&self) -> &[&str];
    fn darkness(&self) -> f32;
    fn objectives(&self) -> Vec<Objective>;
    fn update(&mut self, dt: f32, brain: &BrainInput) -> Vec<EraEvent>;
    fn on_blink(&mut self, aim_x: f32, aim_y: f32) -> Vec<EraEvent>;
    fn on_cast(&mut self, spell: &str, aim_x: f32, aim_y: f32) -> Vec<EraEvent>;
    fn is_complete(&self) -> bool;
    fn is_failed(&self) -> Option<String>;
}
