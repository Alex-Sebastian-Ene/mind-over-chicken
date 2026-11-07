// camera (#33): the seven hand casts. One row per canned MediaPipe gesture.
// `gesture` is the Gesture Recognizer's category name, `key` the keyboard fallback (1–7),
// `cooldown` is per spell, in milliseconds.

export const SPELLS = [
  { id: 'point', key: 1, gesture: 'Pointing_Up', glyph: '☝️', name: 'Aim', cooldown: 0,
    hint: 'Point up: your index fingertip moves the reticle. Key 1 jumps to the next landmark.' },
  { id: 'fireball', key: 2, gesture: 'Closed_Fist', glyph: '✊', name: 'Fireball', cooldown: 1500,
    hint: 'Closed fist: lightning strikes the reticle.' },
  { id: 'spawn', key: 3, gesture: 'Open_Palm', glyph: '🖐️', name: 'Bounty', cooldown: 6000,
    hint: 'Open palm: food, goats or rocket parts, by era.' },
  { id: 'rain', key: 4, gesture: 'Victory', glyph: '✌️', name: 'Rain', cooldown: 10000,
    hint: 'Victory: rain calms fear and boosts the harvest.' },
  { id: 'bless', key: 5, gesture: 'Thumb_Up', glyph: '👍', name: 'Bless', cooldown: 9000,
    hint: 'Thumb up: faith, morale and courage rise.' },
  { id: 'disaster', key: 6, gesture: 'Thumb_Down', glyph: '👎', name: 'Disaster', cooldown: 14000,
    hint: 'Thumb down: a storm hits the reticle.' },
  { id: 'peace', key: 7, gesture: 'ILoveYou', glyph: '🤟', name: 'Peace', cooldown: 15000,
    hint: 'I-love-you: ends strikes, riots and panics.' },
];

export const SPELL_BY_ID = Object.fromEntries(SPELLS.map((s) => [s.id, s]));

// Short readable names for the gesture readout.
export const GESTURE_LABEL = {
  None: 'no gesture',
  Pointing_Up: 'pointing',
  Closed_Fist: 'fist',
  Open_Palm: 'open palm',
  Victory: 'victory',
  Thumb_Up: 'thumb up',
  Thumb_Down: 'thumb down',
  ILoveYou: 'I-love-you',
};

// Digit1..Digit7 and Numpad1..Numpad7 map to the spell with that key.
export function spellForKeyCode(code) {
  const m = /^(?:Digit|Numpad)([1-7])$/.exec(code || '');
  return m ? SPELLS.find((s) => s.key === Number(m[1])) || null : null;
}
