//! Camera casts (#33, #108, #122, #150, #159): the three core hand moves, keys 1-3.
//! This is the table the browser runs (RustCastController); src/camera/spells.js mirrors it for
//! Node. The old seven-cast table (point, spawn, rain, bless, disaster, peace) is gone: its
//! gestures cast nothing, so the panel, coach and keys only show moves that exist.
//!
//! 1. Pinch     grab a living creature under the cursor (landmarks, not a recognizer category).
//! 2. Open palm Calm: held for the hold time, once per hold.
//! 3. Fireball  a closed fist CHARGES it; opening the hand RELEASES it (#150). A held fist alone
//!              never casts. The separate "Closed Fist · Strike" is gone (#159).

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpellId {
    Pinch,
    Palm,
    Fireball,
}

impl SpellId {
    pub fn as_str(&self) -> &'static str {
        match self {
            SpellId::Pinch => "pinch",
            SpellId::Palm => "palm",
            SpellId::Fireball => "fireball",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pinch" => Some(SpellId::Pinch),
            "palm" => Some(SpellId::Palm),
            "fireball" => Some(SpellId::Fireball),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Spell {
    pub id: SpellId,
    pub key: u8,
    /// The recognizer category that holds (or, with `release`, charges) this move.
    pub gesture: &'static str,
    /// Charge-and-release moves: the category that releases the charge. None: a hold move.
    pub release: Option<&'static str>,
    pub glyph: &'static str,
    pub name: &'static str,
    pub cooldown_ms: f64,
    pub aimed: bool,
    pub hint: &'static str,
}

pub const SPELLS: [Spell; 3] = [
    Spell {
        id: SpellId::Pinch,
        key: 1,
        gesture: "Pinch",
        release: None,
        glyph: "🤏",
        name: "Grab",
        cooldown_ms: 400.0,
        aimed: false,
        hint: "Pinch thumb and index: pick up a living creature under the cursor, move the cursor, release to drop.",
    },
    Spell {
        id: SpellId::Palm,
        key: 2,
        gesture: "Open_Palm",
        release: None,
        glyph: "🖐️",
        name: "Calm",
        cooldown_ms: 5000.0,
        aimed: true,
        hint: "Open palm: calm fear, broker peace and bless the land.",
    },
    Spell {
        id: SpellId::Fireball,
        key: 3,
        gesture: "Closed_Fist",
        release: Some("Open_Palm"),
        glyph: "🔥",
        name: "Fireball",
        cooldown_ms: 1500.0,
        aimed: true,
        hint: "Close your fist to charge, open your hand to throw a fireball at the cursor (or hold 3 and release).",
    },
];

/// The move a recognizer category holds or charges. "Open_Palm" is Calm; it is also the release
/// of a charged fireball, which CastController handles before the hold.
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
