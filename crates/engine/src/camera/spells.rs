//! Camera casts (#33, #108): Spells and gestures definitions.
//! One row per canned MediaPipe gesture, keys 1-7.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpellId {
    Point,
    Fireball,
    Spawn,
    Rain,
    Bless,
    Disaster,
    Peace,
}

impl SpellId {
    pub fn as_str(&self) -> &'static str {
        match self {
            SpellId::Point => "point",
            SpellId::Fireball => "fireball",
            SpellId::Spawn => "spawn",
            SpellId::Rain => "rain",
            SpellId::Bless => "bless",
            SpellId::Disaster => "disaster",
            SpellId::Peace => "peace",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "point" => Some(SpellId::Point),
            "fireball" => Some(SpellId::Fireball),
            "spawn" => Some(SpellId::Spawn),
            "rain" => Some(SpellId::Rain),
            "bless" => Some(SpellId::Bless),
            "disaster" => Some(SpellId::Disaster),
            "peace" => Some(SpellId::Peace),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Spell {
    pub id: SpellId,
    pub key: u8,
    pub gesture: &'static str,
    pub glyph: &'static str,
    pub name: &'static str,
    pub cooldown_ms: f64,
    pub aimed: bool,
    pub hint: &'static str,
}

pub const SPELLS: [Spell; 7] = [
    Spell {
        id: SpellId::Point,
        key: 1,
        gesture: "Pointing_Up",
        glyph: "☝️",
        name: "Aim",
        cooldown_ms: 0.0,
        aimed: false,
        hint: "Point up: your index fingertip moves the reticle. Key 1 jumps to the next landmark.",
    },
    Spell {
        id: SpellId::Fireball,
        key: 2,
        gesture: "Closed_Fist",
        glyph: "✊",
        name: "Fireball",
        cooldown_ms: 1500.0,
        aimed: true,
        hint: "Closed fist: lightning strikes the reticle.",
    },
    Spell {
        id: SpellId::Spawn,
        key: 3,
        gesture: "Open_Palm",
        glyph: "🖐️",
        name: "Bounty",
        cooldown_ms: 6000.0,
        aimed: true,
        hint: "Open palm: food, goats or rocket parts, by era.",
    },
    Spell {
        id: SpellId::Rain,
        key: 4,
        gesture: "Victory",
        glyph: "✌️",
        name: "Rain",
        cooldown_ms: 10000.0,
        aimed: true,
        hint: "Victory: rain calms fear and boosts the harvest.",
    },
    Spell {
        id: SpellId::Bless,
        key: 5,
        gesture: "Thumb_Up",
        glyph: "👍",
        name: "Bless",
        cooldown_ms: 9000.0,
        aimed: false,
        hint: "Thumb up: faith, morale and courage rise.",
    },
    Spell {
        id: SpellId::Disaster,
        key: 6,
        gesture: "Thumb_Down",
        glyph: "👎",
        name: "Disaster",
        cooldown_ms: 14000.0,
        aimed: true,
        hint: "Thumb down: a storm hits the reticle.",
    },
    Spell {
        id: SpellId::Peace,
        key: 7,
        gesture: "ILoveYou",
        glyph: "🤟",
        name: "Peace",
        cooldown_ms: 15000.0,
        aimed: false,
        hint: "I-love-you: ends strikes, riots and panics.",
    },
];

pub fn spell_for_gesture(gesture: &str) -> Option<&'static Spell> {
    SPELLS.iter().find(|s| s.gesture == gesture)
}

pub fn spell_for_key(key: u8) -> Option<&'static Spell> {
    SPELLS.iter().find(|s| s.key == key)
}

pub fn spell_for_id(id: SpellId) -> Option<&'static Spell> {
    SPELLS.iter().find(|s| s.id == id)
}

pub fn spell_for_key_code(code: &str) -> Option<&'static Spell> {
    if let Some(digit) = code.strip_prefix("Digit").or_else(|| code.strip_prefix("Numpad")) {
        if let Ok(k) = digit.parse::<u8>() {
            return spell_for_key(k);
        }
    }
    None
}
