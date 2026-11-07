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

/// What the eras read from the player each tick. `stress` is the JS brain's `brain.stress`:
/// EEG agitation or face anger, 0..1, about 0.15 at rest (src/brain/brain.js).
#[derive(Clone, Copy, Debug, Default)]
pub struct BrainInput {
    pub calm: f32,
    pub focus: f32,
    pub stress: f32,
    pub blinks: u32,
}

/// Above this the god counts as stressed (`STRESSED` in src/brain/brain.js).
pub const STRESS_LIMIT: f32 = 0.5;

#[derive(Clone, Copy, Debug)]
pub struct EraBillboard {
    pub sprite_id: u32,
    pub x: f32,
    pub z: f32,
    pub facing: i32,
    pub frame: u32,
    pub variant: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct EraLight {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub radius: f32,
    pub intensity: f32,
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
    fn billboards_3d(&self) -> Vec<EraBillboard> {
        Vec::new()
    }
    fn lights_3d(&self) -> Vec<EraLight> {
        Vec::new()
    }
}
