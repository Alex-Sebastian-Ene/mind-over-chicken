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

#[derive(Clone, Copy, Debug, Default)]
pub struct BrainInput {
    pub calm: f32,
    pub focus: f32,
    pub panic: f32,
    pub hr: f32,
    pub rest_hr: f32,
    pub hr_delta: f32,
    pub blinks: u32,
}

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
